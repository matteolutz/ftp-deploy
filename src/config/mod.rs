use std::{fs, path::Path};

use serde::{Serialize, de::DeserializeOwned};

mod ftp;
pub use ftp::*;

mod creds;
pub use creds::*;

pub trait ConfigLoader {
    fn create(base_path: impl AsRef<Path>) -> Result<(Self, bool), Box<dyn std::error::Error>>
    where
        Self: Sized;

    fn load_or_create(base_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized;
}

pub trait Config: Default + Serialize + DeserializeOwned {
    const FILE_NAME: &'static str;
}

impl<T: Config> ConfigLoader for T {
    fn create(base_path: impl AsRef<Path>) -> Result<(Self, bool), Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let file_path = base_path.as_ref().join(Self::FILE_NAME);

        if file_path.exists() {
            println!(
                "[ftp-deploy] Config file '{}' already exists. Skipping creation.",
                Self::FILE_NAME
            );
            return Ok((Self::default(), false));
        }

        let config = Self::default();

        let file = fs::File::create(file_path)?;
        serde_json::to_writer_pretty(file, &config)?;

        Ok((config, true))
    }

    fn load_or_create(base_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let file_path = base_path.as_ref().join(Self::FILE_NAME);

        if file_path.exists() {
            let file = fs::File::open(file_path)?;
            let config = serde_json::from_reader(file)?;
            return Ok(config);
        }

        println!(
            "[ftp-deploy] Config file '{}' not found, creating it.",
            Self::FILE_NAME
        );

        Self::create(base_path).map(|(config, _)| config)
    }
}
