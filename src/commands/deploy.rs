use std::{
    collections::HashMap,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time,
};

use clap::Args;
use ignore::WalkBuilder;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};

use crate::{
    commands::SubcommandDelegate,
    config::{ConfigLoader, FtpConfig, FtpCreds},
    ftp::FtpStreamExt,
    tracking::{FilesTracking, IGNORE_FILE_NAME, TrackingFileLoder},
};

#[derive(Clone)]
struct FileWalk {
    files: Arc<RwLock<HashMap<PathBuf, String>>>,
    updated_files: Arc<Mutex<Vec<PathBuf>>>,
}

impl FileWalk {
    fn insert_update(&self, path: PathBuf, hash: String) {
        self.files.write().unwrap().insert(path.clone(), hash);
        self.updated_files.lock().unwrap().push(path);
    }

    fn update(&self, path: impl AsRef<Path>, hash: String, force: bool) {
        if self.files.read().unwrap().contains_key(path.as_ref()) {
            if force || self.files.read().unwrap().get(path.as_ref()).unwrap() != &hash {
                self.insert_update(path.as_ref().to_path_buf(), hash);
            }
        } else {
            self.insert_update(path.as_ref().to_path_buf(), hash);
        }
    }
}

impl From<FilesTracking> for FileWalk {
    fn from(value: FilesTracking) -> Self {
        Self {
            files: Arc::new(RwLock::new(value.files)),
            updated_files: Arc::new(Mutex::new(vec![])),
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
    pub fn from_updated_and_deleted(
        updated_files: Vec<PathBuf>,
        deleted_files: Vec<PathBuf>,
    ) -> Vec<Self> {
        let mut updates = Vec::with_capacity(updated_files.len() + deleted_files.len());

        for file in updated_files {
            updates.push(FileUpdate {
                file,
                update_type: FileUpdateType::CreateOrUpdate,
            });
        }

        for file in deleted_files {
            updates.push(FileUpdate {
                file,
                update_type: FileUpdateType::Deleted,
            });
        }

        updates
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
    ) -> Result<(HashMap<PathBuf, String>, Vec<PathBuf>), Box<dyn std::error::Error>> {
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
        let updated_files = Arc::try_unwrap(file_walk.updated_files)
            .unwrap()
            .into_inner()?;

        println!("[ftp-deploy] Collecting files took {:?}.", start.elapsed(),);

        Ok((files, updated_files))
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
        let files_before = files_tracking
            .files()
            .keys()
            .clone()
            .into_iter()
            .map(|path| path.clone())
            .collect::<Vec<_>>();

        let (files, updated_files) = self.collect_files(&base_path, files_tracking)?;

        let deleted_files = files_before
            .into_iter()
            .filter(|path| !files.contains_key(path.as_path()))
            .collect::<Vec<_>>();

        println!(
            "[ftp-deploy] {} file(s) were updated, {} file(s) were deleted",
            updated_files.len(),
            deleted_files.len()
        );

        if self.debug {
            println!("Updated files: {:?}", updated_files);
            println!("Deleted files: {:?}", deleted_files);
        }

        let updates = FileUpdate::from_updated_and_deleted(updated_files, deleted_files);

        if !self.dry {
            if !updates.is_empty() {
                self.upload_files(&creds, updates)?;
            } else {
                println!("[ftp-deploy] No files to upload.")
            }

            let files_tracking = FilesTracking { files };
            files_tracking.write(&base_path)?;
        }

        Ok(())
    }
}
