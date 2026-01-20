use std::path::PathBuf;

use clap::Args;
use ignore::WalkBuilder;

use crate::{commands::SubcommandDelegate, tracking::IGNORE_FILE_NAME};

#[derive(Args)]
pub struct FilesCommand {
    /// Directory to initialize the configuration file in
    #[arg(short, long)]
    path: Option<PathBuf>,
}

impl SubcommandDelegate for FilesCommand {
    fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let base_path = self.path.unwrap_or_else(|| PathBuf::from("."));

        for f in WalkBuilder::new(&base_path)
            .add_custom_ignore_filename(IGNORE_FILE_NAME)
            .build()
            .filter_map(|res| res.ok())
        {
            println!("[ftp-deploy] {}", f.path().display());
        }

        Ok(())
    }
}
