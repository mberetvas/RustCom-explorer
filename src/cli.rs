use clap::{Parser, Subcommand, Args as ClapArgs};

/// RustCOM Explorer CLI
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List available COM objects
    List(ListArgs),
}

#[derive(ClapArgs, Debug)]
pub struct ListArgs {
    /// Filter objects by name, CLSID, or description
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,

    /// Save output to a file path
    #[arg(short, long)]
    pub output: Option<String>,
}