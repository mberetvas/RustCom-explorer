// src/main.rs
use std::io;
use std::fmt::Write as FmtWrite; // Import for writing to String buffer
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
    /// Initializes the terminal: raw mode, alternate screen, panic hook.
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        // Setup panic hook to restore terminal before printing panic info
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

fn main() -> Result<()> {
    // 1. Parse CLI Arguments
    let args = Args::parse();

    // 2. Initialize COM Library (Required for both CLI and TUI)
    // We do this early to ensure COM is ready.
    let _com_guard = com_interop::initialize_com()?;
    
    // 3. Branch based on Command
    match args.command {
        Some(Commands::List(list_args)) => {
            // --- CLI Mode: List ---
            
            // A. Scan for Objects
            let objects = match scanner::scan_com_objects() {
                Ok(objs) => objs,
                Err(e) => {
                    eprintln!("Error: Failed to scan COM objects: {:#}", e);
                    std::process::exit(1);
                }
            };
            
            // B. Filter and Process
            // If filter is None, we pass an empty string to match all
            let filter_query = list_args.filter.as_deref().unwrap_or("");
            let grouped_objects = comm_browser::processor::process_objects(objects, filter_query);

            // C. Format Output
            let output_content = if list_args.json {
                // JSON Formatting
                serde_json::to_string_pretty(&grouped_objects)
                    .expect("Failed to serialize COM objects to JSON")
            } else {
                // Text Formatting
                let mut buffer = String::new();
                for (category, objects) in grouped_objects {
                    // We handle fmt error by unwrap since writing to String shouldn't fail
                    writeln!(&mut buffer, "[{}]", category).unwrap();
                    for obj in objects {
                        writeln!(
                            &mut buffer, 
                            "  {} ({}) - {}", 
                            obj.name, obj.clsid, obj.description
                        ).unwrap();
                    }
                }
                buffer
            };

            // D. Output Handling (File vs Stdout)
            if let Some(path) = list_args.output {
                if let Err(e) = std::fs::write(&path, output_content) {
                    eprintln!("Error: Failed to write output to file '{}': {:#}", path, e);
                    std::process::exit(1);
                } else {
                    println!("Successfully wrote output to '{}'", path);
                }
            } else {
                println!("{}", output_content);
            }
        }
        None => {
            // --- TUI Mode (Default) ---
            println!("Scanning for COM objects... (This may take a moment)");
            
            // Scan for objects (blocking for now, before TUI starts)
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

            // Initialize TUI
            let mut tui = Tui::new()?;

            // Run App
            let mut app = App::new(objects);
            app.run(&mut tui.terminal)?;
        }
    }

    Ok(())
}