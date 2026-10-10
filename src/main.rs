#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

mod import;
mod launch;
mod mcp;
mod setup;
#[allow(dead_code)]
mod store;
mod ui;

use std::path::PathBuf;

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
    ImportTrello {
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

fn import_trello(dir: Option<PathBuf>) -> Result<(), String> {
    let dir = dir.unwrap_or_else(import::trello_dir);
    let mut store = store::Store::open_default().map_err(|e| e.to_string())?;
    let report = import::import_all(&mut store, &dir)?;
    for (key, name, cards) in &report.imported {
        println!("imported {name} as {key}: {cards} cards");
    }
    for (name, reason) in &report.skipped {
        println!("skipped {name}: {reason}");
    }
    Ok(())
}

#[cfg(windows)]
fn attach_parent_console() {
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

fn main() {
    #[cfg(windows)]
    if std::env::args_os().nth(1).is_some_and(|arg| arg != "mcp") {
        attach_parent_console();
    }
    let cli = Cli::parse();
    if !matches!(cli.command, Some(Command::Mcp { .. }))
        && std::env::var_os("LOCALAPPDATA").is_some()
    {
        setup::install::cleanup_old(&setup::install::bin_dir());
        setup::install::cleanup_old(&setup::install::gui_dir());
    }
    match cli.command {
        None => {
            ui::install_panic_hook();
            ui::run();
        }
        Some(Command::Mcp { project }) => mcp::run(&project),
        Some(Command::Install) => match setup::install::install() {
            Ok(paths) => {
                for path in paths {
                    println!("installed {}", path.display());
                }
                let installed = setup::agents::install_agents(&setup::agents::agents_dir())
                    .and_then(|mut agents| {
                        agents.extend(setup::skills::install_skills(&setup::skills::skills_dir())?);
                        Ok(agents)
                    });
                match installed {
                    Ok(actions) => {
                        for (file, action) in actions {
                            println!("{file}: {action}");
                        }
                    }
                    Err(e) => {
                        eprintln!("agent files failed: {e}");
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("install failed: {e}");
                std::process::exit(1);
            }
        },
        Some(Command::ImportTrello { dir }) => {
            if let Err(e) = import_trello(dir) {
                eprintln!("import failed: {e}");
                std::process::exit(1);
            }
        }
    }
}
