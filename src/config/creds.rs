use std::path::{Path, PathBuf};

use ftp::FtpStream;
use serde_derive::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FtpCreds {
    pub server: String,
    pub base_path: PathBuf,
    pub username: String,
    pub password: String,
}

impl FtpCreds {
    pub fn open_stream(&self) -> Result<FtpStream, Box<dyn std::error::Error>> {
        let mut ftp_stream = FtpStream::connect(&self.server)?;
        ftp_stream.login(&self.username, &self.password)?;
        Ok(ftp_stream)
    }

    pub fn ftp_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.base_path.join(path)
    }
}

impl Config for FtpCreds {
    const FILE_NAME: &'static str = "ftp-deploy-creds.json";
}
