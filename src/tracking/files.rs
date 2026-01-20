use std::{collections::HashMap, path::PathBuf};

use serde_derive::{Deserialize, Serialize};

use crate::tracking::TrackingFile;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileState {
    File(String),
    Directory,
}

#[derive(Default, Serialize, Deserialize)]
pub struct FilesTracking {
    pub(crate) files: HashMap<PathBuf, FileState>,
}

impl TrackingFile for FilesTracking {
    const FILE_NAME: &'static str = "files.json";
}
