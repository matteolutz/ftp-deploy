use std::{
    collections::HashMap,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time,
};

use clap::Args;
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use sha2::{Digest, Sha256};

use crate::{
    commands::SubcommandDelegate,
    config::{ConfigLoader, FtpConfig, FtpCreds},
    ftp::FtpStreamExt,
    tracking::{FilesTracking, IGNORE_FILE_NAME, TrackingFileLoder},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum FileMode {
    Untouched,
    Created,
    Updated,
    Deleted,
}

#[derive(Clone)]
struct FileWalk {
    files: Arc<RwLock<HashMap<PathBuf, (String, FileMode)>>>,
}

impl FileWalk {
    fn insert_update(&self, path: PathBuf, hash: String, mode: FileMode) {
        self.files
            .write()
            .unwrap()
            .insert(path.clone(), (hash, mode));
    }

    fn update(&self, path: impl AsRef<Path>, hash: String, force: bool) {
        if self.files.read().unwrap().contains_key(path.as_ref()) {
            let mode = if force || self.files.read().unwrap().get(path.as_ref()).unwrap().0 != hash
            {
                FileMode::Updated
            } else {
                FileMode::Untouched
            };

            self.insert_update(path.as_ref().to_path_buf(), hash, mode);
        } else {
            self.insert_update(path.as_ref().to_path_buf(), hash, FileMode::Created);
        }
    }
}

impl From<FilesTracking> for FileWalk {
    fn from(value: FilesTracking) -> Self {
        Self {
            files: Arc::new(RwLock::new(
                value
                    .files
                    .into_iter()
                    .map(|(key, value)| (key, (value, FileMode::Deleted)))
                    .collect(),
            )),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum FileUpdateType {
    CreateOrUpdate,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileUpdate {
    file: PathBuf,
    update_type: FileUpdateType,
}

impl FileUpdate {
    pub fn from_files(files: &HashMap<PathBuf, (String, FileMode)>) -> Vec<FileUpdate> {
        files
            .iter()
            .filter_map(|(path, (_, mode))| {
                let update_mode = match mode {
                    FileMode::Created | FileMode::Updated => FileUpdateType::CreateOrUpdate,
                    FileMode::Deleted => FileUpdateType::Deleted,
                    _ => return None,
                };

                Some(FileUpdate {
                    file: path.clone(),
                    update_type: update_mode,
                })
            })
            .collect()
    }
}

#[derive(Args)]
pub struct DeployCommand {
    /// Directory to initialize the configuration file in
    #[arg(short, long)]
    path: Option<PathBuf>,

    /// Number of threads to use for walking files
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Force deploy even if no changes are detected
    #[arg(short, long)]
    force: bool,

    /// Dry run, do not actually deploy
    #[arg(short, long)]
    dry: bool,

    /// Debug mode, print additional information
    #[arg(short, long)]
    debug: bool,
}

impl DeployCommand {
    fn collect_files(
        &self,
        base_path: &Path,
        files_tracking: FilesTracking,
    ) -> Result<HashMap<PathBuf, (String, FileMode)>, Box<dyn std::error::Error>> {
        let jobs = self.jobs.unwrap_or_else(|| num_cpus::get());
        let file_walk: FileWalk = files_tracking.into();

        let walker = WalkBuilder::new(&base_path)
            .add_custom_ignore_filename(IGNORE_FILE_NAME)
            .hidden(false)
            .threads(jobs)
            .build_parallel();

        println!("[ftp-deploy] Collecting files using {} threads", jobs);
        let start = time::Instant::now();

        walker.run(|| {
            let file_walk = file_walk.clone();
            let force = self.force;

            Box::new(move |result| {
                let Ok(result) = result else {
                    return ignore::WalkState::Continue;
                };

                let path = result.path();
                if path.is_file() {
                    let mut hasher = Sha256::new();
                    let mut file = fs::File::open(path).unwrap();
                    io::copy(&mut file, &mut hasher).unwrap();

                    file_walk.update(path, format!("{:x}", hasher.finalize()), force);
                }

                ignore::WalkState::Continue
            })
        });

        let files = Arc::try_unwrap(file_walk.files).unwrap().into_inner()?;
        println!("[ftp-deploy] Collecting files took {:?}.", start.elapsed(),);

        Ok(files)
    }

    fn upload_files(
        &self,
        creds: &FtpCreds,
        updated_files: Vec<FileUpdate>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("[ftp-deploy] Uploading files to {}", creds.server);

        let mut ftp_stream = creds.open_stream()?;

        ftp_stream.cwd("/")?;
        let mut _current_ftp_path = PathBuf::from("/");

        let style = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] {msg} [{wide_bar:.cyan/blue}] ({eta})",
        )
        .unwrap()
        .progress_chars("#>-");
        let pb = ProgressBar::new(updated_files.len() as u64).with_style(style);

        for FileUpdate { file, update_type } in updated_files.into_iter() {
            // TODO: sort file paths and only do necessary mkdir's and cwd's

            let Some(file_name) = file.file_name() else {
                println!("[ftp-deploy] Skipping invalid file {}", file.display());
                continue;
            };

            let ftp_path = creds.ftp_path(&file);
            // TODO: get relative path to current path

            let file_name: &str = file_name.try_into().unwrap();

            pb.set_message(file.display().to_string());

            ftp_stream.cwd_or_create_recursive(ftp_path.parent())?;

            // TODO: update current path

            match update_type {
                FileUpdateType::Deleted => {
                    if file.is_file() {
                        ftp_stream.rm(file_name)?;
                    } else {
                        ftp_stream.rmdir(file_name)?;
                    }
                }
                FileUpdateType::CreateOrUpdate => {
                    let mut reader = File::open(&file)?;
                    ftp_stream.put(file_name, &mut reader)?;
                }
            }

            pb.inc(1);
        }

        Ok(())
    }
}

impl SubcommandDelegate for DeployCommand {
    fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let base_path = self.path.clone().unwrap_or_else(|| PathBuf::from("."));

        let config = FtpConfig::load_or_create(&base_path)?;
        let creds = FtpCreds::load_or_create(&base_path)?;

        if !config.hooks().is_empty() {
            println!("[ftp-deploy] Running {} hook(s)", config.hooks().len());
            config.run_hooks();
        }

        let files_tracking = FilesTracking::load_or_create(&base_path)?;

        let files = self.collect_files(&base_path, files_tracking)?;

        println!(
            "[ftp-deploy] {} file(s) created, {} file(s) updated, {} file(s) were deleted",
            files
                .iter()
                .filter(|(_, (_, mode))| *mode == FileMode::Created)
                .count(),
            files
                .iter()
                .filter(|(_, (_, mode))| *mode == FileMode::Updated)
                .count(),
            files
                .iter()
                .filter(|(_, (_, mode))| *mode == FileMode::Deleted)
                .count(),
        );

        if self.debug {
            println!(
                "{}",
                files
                    .iter()
                    .map(|(path, (_, mode))| format!("{}: {:?}", path.display(), mode))
                    .join("\n")
            );
        }

        let updates = FileUpdate::from_files(&files);
        let files_tracking = FilesTracking {
            files: files
                .into_iter()
                .map(|(path, (hash, _))| (path, hash))
                .collect(),
        };

        if !self.dry {
            if !updates.is_empty() {
                self.upload_files(&creds, updates)?;
            } else {
                println!("[ftp-deploy] No files to upload.")
            }

            files_tracking.write(&base_path)?;
        }

        Ok(())
    }
}
