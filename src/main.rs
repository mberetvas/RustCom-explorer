use std::io;
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
            // Placeholder logic for Phase 2
            println!("List command executed.");
            if let Some(filter) = list_args.filter {
                println!("Filter applied: {}", filter);
            }
            if list_args.json {
                println!("Output format: JSON");
            }
            if let Some(path) = list_args.output {
                println!("Output file: {}", path);
            }
            
            // In the future, we will call scanner::scan_com_objects() here
            // and filter/serialize the results.
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