use clap::{Parser, Subcommand};

use crate::commands::{DeployCommand, FilesCommand, InitCommand, SubcommandDelegate};

mod commands;
mod config;
mod ftp;
mod tracking;

#[derive(Subcommand)]
enum Command {
    /// Initialize a new configuration file
    Init(InitCommand),

    /// Deploy files to a remote server
    Deploy(DeployCommand),

    /// List all tracked files
    Files(FilesCommand),
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.command {
        Command::Init(init) => init.run(),
        Command::Deploy(deploy) => deploy.run(),
        Command::Files(files) => files.run(),
    }?;

    Ok(())
}
