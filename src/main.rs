mod import;
mod mcp;
mod setup;
#[allow(dead_code)]
mod store;
mod ui;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fuel-station")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Mcp {
        #[arg(long)]
        project: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        None => ui::run(),
        Some(Command::Mcp { project }) => mcp::run(&project),
    }
}
