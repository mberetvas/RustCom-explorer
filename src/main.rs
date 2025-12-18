// src/main.rs
use std::io;
use std::fmt::Write as FmtWrite; 
use std::path::PathBuf;
use std::collections::BTreeMap;
use serde::Serialize;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use clap::Parser;
use rustcom_explorer::{app::App, com_interop, scanner, error_handling::Result, cli::{Args, Commands}};

// Parallelism & COM Imports
use rayon::prelude::*;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

/// RAII wrapper for TUI terminal setup and teardown.
pub struct Tui {
    pub terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl Tui {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            original_hook(panic_info);
        }));

        Ok(Self { terminal })
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

/// Helper struct for JSON export that includes deep inspection details.
#[derive(Serialize)]
struct EnhancedComObject {
    #[serde(flatten)]
    base: scanner::ComObject,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<com_interop::TypeDetails>,
}

/// Configures the Rayon global thread pool with COM initialization.
fn configure_rayon_pool() -> Result<()> {
    rayon::ThreadPoolBuilder::new()
        .start_handler(|_| unsafe {
            // Initialize COM as Multi-Threaded on each worker thread
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        })
        .exit_handler(|_| unsafe {
            CoUninitialize();
        })
        .build_global()
        .map_err(|e| anyhow::anyhow!("Failed to configure Rayon thread pool: {}", e))
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    // Logging and Verbosity
    if args.verbose {
        eprintln!("[INFO] Verbose logging enabled.");
        if args.unsafe_mode {
            eprintln!("[WARN] UNSAFE MODE ENABLED. Objects missing Type Libraries will be instantiated.");
        } else {
            eprintln!("[INFO] Safety mode engaged. Only Registry Type Libraries will be inspected.");
        }
    }

    // 1. Configure Thread Pool (Must be done before any parallel ops)
    configure_rayon_pool()?;

    // 2. Main Thread COM Init
    let _com_guard = com_interop::initialize_com()?;
    
    match args.command {
        Some(Commands::List(list_args)) => {
            // --- CLI Mode: List ---
            
            if args.verbose {
                eprintln!("[INFO] Scanning Registry for COM Objects...");
            }

            // A. Scan
            let objects = match scanner::scan_com_objects() {
                Ok(objs) => objs,
                Err(e) => {
                    eprintln!("Error: Failed to scan COM objects: {:#}", e);
                    std::process::exit(1);
                }
            };
            
            if args.verbose {
                eprintln!("[INFO] Found {} objects. Filtering...", objects.len());
            }

            // B. Filter
            let filter_query = list_args.filter.as_deref().unwrap_or("");
            let grouped_objects = rustcom_explorer::processor::process_objects(objects, filter_query);

            // C. Format
            let (output_content, ext) = if list_args.json {
                let mut enhanced_groups = BTreeMap::new();
                
                // 1. Flatten the grouped structure for parallel processing
                let flat_objects: Vec<(String, scanner::ComObject)> = grouped_objects
                    .into_iter()
                    .flat_map(|(cat, objs)| objs.into_iter().map(move |obj| (cat.clone(), obj)))
                    .collect();

                let total_objects = flat_objects.len();
                let num_threads = rayon::current_num_threads();
                
                // UI: Progress Feedback
                eprintln!("Processing {} objects on {} threads...", total_objects, num_threads);

                // 2. Parallel Deep Inspection
                let allow_unsafe = args.unsafe_mode;
                
                let enhanced_flat: Vec<(String, EnhancedComObject)> = flat_objects
                    .into_par_iter()
                    .map(|(category, obj)| {
                        // Perform the COM/Registry lookup here, respecting safety flag
                        let details = com_interop::get_type_info(&obj.clsid, allow_unsafe).ok();
                        
                        (category, EnhancedComObject {
                            base: obj,
                            details,
                        })
                    })
                    .collect();

                // 3. Re-group into BTreeMap
                for (category, obj) in enhanced_flat {
                    enhanced_groups.entry(category).or_insert_with(Vec::new).push(obj);
                }

                (
                    serde_json::to_string_pretty(&enhanced_groups)
                        .expect("Failed to serialize COM objects to JSON"),
                    "json"
                )
            } else {
                let mut buffer = String::new();
                for (category, objects) in grouped_objects {
                    writeln!(&mut buffer, "[{}]", category).unwrap();
                    for obj in objects {
                        writeln!(
                            &mut buffer, 
                            "  {} ({}) - {}", 
                            obj.name, obj.clsid, obj.description
                        ).unwrap();
                    }
                }
                (buffer, "txt")
            };

            // D. Output to File
            if let Some(raw_path) = list_args.output {
                let mut path = PathBuf::from(raw_path);
                
                // Smart Extension Handling:
                let should_append = path.file_name()
                    .map(|name| {
                        !name.to_string_lossy()
                            .to_lowercase()
                            .ends_with(&format!(".{}", ext))
                    })
                    .unwrap_or(false);

                if should_append
                    && let Some(stem) = path.file_name() {
                        let mut new_name = stem.to_os_string();
                        new_name.push(format!(".{}", ext));
                        path.set_file_name(new_name);
                    }

                if let Err(e) = std::fs::write(&path, output_content) {
                    eprintln!("Error: Failed to write output to file '{}': {:#}", path.display(), e);
                    std::process::exit(1);
                } else {
                    println!("Successfully wrote report to '{}'", path.display());
                }
            } else {
                println!("{}", output_content);
            }
        }
        None => {
            // --- TUI Mode ---
            if args.verbose {
                eprintln!("[INFO] Starting TUI Mode...");
            }

            println!("Scanning for COM objects... (This may take a moment)");
            let objects = match scanner::scan_com_objects() {
                Ok(objs) => objs,
                Err(e) => {
                    eprintln!("Failed to scan COM objects: {:?}", e);
                    return Err(e);
                }
            };

            if objects.is_empty() {
                println!("No COM objects found. Press Enter to exit.");
                let mut line = String::new();
                let _ = std::io::stdin().read_line(&mut line);
                return Ok(());
            }

            let mut tui = Tui::new()?;
            let mut app = App::new(objects, args.unsafe_mode);
            app.run(&mut tui.terminal)?;
        }
    }

    Ok(())
}