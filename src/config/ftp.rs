#[cfg(unix)]
use std::process::Command;

use serde_derive::{Deserialize, Serialize};

use crate::config::Config;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FtpConfig {
    hooks: Vec<String>,
}

impl FtpConfig {
    pub fn hooks(&self) -> &[String] {
        &self.hooks
    }

    pub fn run_hooks(&self) {
        for hook in &self.hooks {
            println!("[ftp-deploy] Running hook: \"{}\"", hook);

            #[cfg(unix)]
            let output = Command::new("sh").arg("-c").arg(hook).output();
            #[cfg(windows)]
            let output = Command::new("cmd").arg("/C").arg(hook).output();

            let Ok(output) = output else {
                println!("[ftp-deploy] Failed to run hook");
                return;
            };

            if !output.stdout.is_empty() {
                println!(
                    "[ftp-deploy] Hook output: {}",
                    String::from_utf8_lossy(&output.stdout)
                );
            }

            if !output.stderr.is_empty() {
                println!(
                    "[ftp-deploy] Hook error: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            if !output.status.success() {
                println!("[ftp-deploy] Hook failed");
            }
        }
    }
}

impl Config for FtpConfig {
    const FILE_NAME: &'static str = "ftp-deploy.json";
}
