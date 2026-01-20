use std::{fs, io::Write, path::Path};

use serde::{Serialize, de::DeserializeOwned};

mod files;
pub use files::*;

pub const IGNORE_FILE_NAME: &str = ".ftpignore";

pub fn create_ignore_file(base_path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let file_path = base_path.as_ref().join(IGNORE_FILE_NAME);

    if file_path.exists() {
        println!(
            "[ftp-deploy] Ignore file '{}' already exists. Skipping creation.",
            IGNORE_FILE_NAME
        );
        return Ok(());
    }

    let mut file = fs::File::create(file_path)?;
    file.write_all(".ftp/".as_bytes())?;

    Ok(())
}

const TRACKING_DIR: &str = ".ftp";

pub fn create_tracking_dir(base_path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let tracking_dir = base_path.as_ref().join(TRACKING_DIR);
    fs::create_dir_all(&tracking_dir)?;
    Ok(())
}

pub trait TrackingFileLoder {
    fn load_or_create(base_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized;

    fn write(&self, base_path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>>;
}

pub trait TrackingFile: Default + Serialize + DeserializeOwned {
    const FILE_NAME: &'static str;
}

impl<T: TrackingFile> TrackingFileLoder for T {
    fn load_or_create(base_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let file_path = base_path.as_ref().join(".ftp/").join(Self::FILE_NAME);

        if file_path.exists() {
            let file = fs::File::open(file_path)?;
            let config = serde_json::from_reader(file)?;
            return Ok(config);
        }

        println!(
            "[ftp-deploy] Tracking file '{}' not found, creating it.",
            Self::FILE_NAME
        );

        let config = Self::default();

        fs::create_dir_all(file_path.parent().unwrap())?;

        let file = fs::File::create(file_path)?;
        serde_json::to_writer(file, &config)?;

        Ok(config)
    }

    fn write(&self, base_path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        let file_path = base_path.as_ref().join(".ftp/").join(Self::FILE_NAME);

        fs::create_dir_all(file_path.parent().unwrap())?;
        let file = fs::File::create(file_path)?;

        serde_json::to_writer(file, &self)?;
        Ok(())
    }
}
