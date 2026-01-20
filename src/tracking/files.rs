use std::{collections::HashMap, path::PathBuf};

use serde_derive::{Deserialize, Serialize};

use crate::tracking::TrackingFile;

#[derive(Default, Serialize, Deserialize)]
pub struct FilesTracking {
    pub(crate) files: HashMap<PathBuf, String>,
}

impl FilesTracking {
    pub fn files(&self) -> &HashMap<PathBuf, String> {
        &self.files
    }
}

impl TrackingFile for FilesTracking {
    const FILE_NAME: &'static str = "files.json";
}
