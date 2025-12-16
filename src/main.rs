use std::io;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use comm_browser::{app::App, com_interop, scanner, error_handling::Result};

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
    // 1. Initialize COM Library
    let _com_guard = com_interop::initialize_com()?;
    
    println!("Scanning for COM objects... (This may take a moment)");
    
    // 2. Scan for objects (blocking for now, before TUI starts)
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

    // 3. Initialize TUI
    let mut tui = Tui::new()?;

    // 4. Run App
    let mut app = App::new(objects);
    app.run(&mut tui.terminal)?;

    Ok(())
}