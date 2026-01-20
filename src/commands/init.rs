use std::{fs, path::PathBuf};

use clap::Args;

use crate::{
    commands::SubcommandDelegate,
    config::{ConfigLoader, FtpConfig, FtpCreds},
    tracking::{create_ignore_file, create_tracking_dir},
};

#[derive(Args)]
pub struct InitCommand {
    /// Directory to initialize the configuration file in
    #[arg(short, long)]
    path: Option<PathBuf>,
}

impl SubcommandDelegate for InitCommand {
    fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let base_path = self.path.unwrap_or_else(|| PathBuf::from("."));

        println!(
            "[ftp-deploy] Initializing in \"{}\"",
            fs::canonicalize(&base_path)?.display()
        );

        FtpConfig::create(&base_path)?;
        FtpCreds::create(&base_path)?;
        create_ignore_file(&base_path)?;
        create_tracking_dir(&base_path)?;

        println!("[ftp-deploy] Done.");

        Ok(())
    }
}
