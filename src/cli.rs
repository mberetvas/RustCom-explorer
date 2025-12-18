// src/cli.rs
use clap::{Parser, Subcommand, Args as ClapArgs};

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

    /// Output in JSON format instead of the default Text format
    #[arg(long)]
    pub json: bool,

    /// Output to a specific file (extension will be added automatically)
    #[arg(short, long)]
    pub output: Option<String>,
}