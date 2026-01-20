use std::path::{Component, Path};

use ftp::{FtpError, FtpStream};

pub trait FtpStreamExt {
    fn cwd_or_create_recursive(
        &mut self,
        directory: Option<impl AsRef<Path>>,
    ) -> Result<(), FtpError>;
}

impl FtpStreamExt for FtpStream {
    fn cwd_or_create_recursive(
        &mut self,
        directory: Option<impl AsRef<Path>>,
    ) -> Result<(), FtpError> {
        let Some(directory) = directory else {
            return self.cwd("/");
        };

        for component in directory.as_ref().components() {
            match component {
                Component::RootDir => self.cwd("/")?,
                Component::CurDir => {}
                Component::ParentDir => self.cwd("..")?,
                Component::Normal(name) => {
                    let name: &str = name.try_into().unwrap();

                    let _ = self.mkdir(name);
                    self.cwd(name)?;
                }
                Component::Prefix(_) => {}
            }
        }

        Ok(())
    }
}
