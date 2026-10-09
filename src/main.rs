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
    Install,
}

fn main() {
    let cli = Cli::parse();
    if !matches!(cli.command, Some(Command::Mcp { .. }))
        && std::env::var_os("LOCALAPPDATA").is_some()
    {
        setup::install::cleanup_old(&setup::install::bin_dir());
    }
    match cli.command {
        None => ui::run(),
        Some(Command::Mcp { project }) => mcp::run(&project),
        Some(Command::Install) => match setup::install::install() {
            Ok(path) => println!("installed {}", path.display()),
            Err(e) => {
                eprintln!("install failed: {e}");
                std::process::exit(1);
            }
        },
    }
}
