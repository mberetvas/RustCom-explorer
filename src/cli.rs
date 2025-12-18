// src/cli.rs
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Enable unsafe COM object instantiation for deep inspection.
    /// Warning: This may start external services or cause side effects.
    #[arg(long = "unsafe", global = true, default_value_t = false)]
    pub unsafe_mode: bool,

    /// Enable verbose output logging.
    #[arg(short, long, global = true, default_value_t = false)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List available COM objects
    List(ListArgs),
}

#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Filter objects by name or CLSID
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Output to file (auto-detects extension)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Export as JSON with deep inspection details
    #[arg(long)]
    pub json: bool,
}