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
use comm_browser::{app::App, com_interop, scanner, error_handling::Result, cli::{Args, Commands}};

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

fn main() -> Result<()> {
    let args = Args::parse();
    let _com_guard = com_interop::initialize_com()?;
    
    match args.command {
        Some(Commands::List(list_args)) => {
            // --- CLI Mode: List ---
            
            // A. Scan
            let objects = match scanner::scan_com_objects() {
                Ok(objs) => objs,
                Err(e) => {
                    eprintln!("Error: Failed to scan COM objects: {:#}", e);
                    std::process::exit(1);
                }
            };
            
            // B. Filter
            let filter_query = list_args.filter.as_deref().unwrap_or("");
            let grouped_objects = comm_browser::processor::process_objects(objects, filter_query);

            // C. Format
            let (output_content, ext) = if list_args.json {
                let mut enhanced_groups = BTreeMap::new();
                
                // Warn user if inspecting a large number of objects as it involves heavy COM/Registry calls
                let total_objects: usize = grouped_objects.values().map(|v| v.len()).sum();
                if total_objects > 50 {
                    eprintln!("Inspecting {} objects for details... This may take a while.", total_objects);
                }

                for (category, objects) in grouped_objects {
                    let mut enhanced_list = Vec::with_capacity(objects.len());
                    for obj in objects {
                        // Attempt to fetch full type info (members, properties, etc.)
                        let details = com_interop::get_type_info(&obj.clsid).ok();
                        enhanced_list.push(EnhancedComObject {
                            base: obj,
                            details,
                        });
                    }
                    enhanced_groups.insert(category, enhanced_list);
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
            let mut app = App::new(objects);
            app.run(&mut tui.terminal)?;
        }
    }

    Ok(())
}