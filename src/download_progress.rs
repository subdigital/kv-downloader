use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct ProgressData {
    url: String,
    completed_tracks: Vec<String>,
}

pub struct DownloadProgress {
    progress_file: PathBuf,
}

impl DownloadProgress {
    pub fn new() -> Self {
        Self {
            progress_file: PathBuf::from(".kv_download_progress.json"),
        }
    }

    pub fn new_with_path(download_path: Option<&str>) -> Self {
        let progress_file = if let Some(path) = download_path {
            PathBuf::from(path).join(".kv_download_progress.json")
        } else {
            PathBuf::from(".kv_download_progress.json")
        };

        Self { progress_file }
    }

    pub fn is_track_downloaded(&self, track_name: &str) -> Result<bool> {
        if !self.progress_file.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(&self.progress_file)?;
        let progress: ProgressData = serde_json::from_str(&content)?;

        Ok(progress.completed_tracks.contains(&track_name.to_string()))
    }

    pub fn mark_track_downloaded(&self, track_name: &str) -> Result<()> {
        let mut progress = self.load_or_create()?;

        if !progress.completed_tracks.contains(&track_name.to_string()) {
            progress.completed_tracks.push(track_name.to_string());
            self.save(&progress)?;
        }

        Ok(())
    }

    pub fn set_url(&self, url: &str) -> Result<()> {
        let mut progress = self.load_or_create()?;
        progress.url = url.to_string();
        self.save(&progress)?;
        Ok(())
    }

    pub fn is_same_url(&self, url: &str) -> Result<bool> {
        if !self.progress_file.exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(&self.progress_file)?;
        let progress: ProgressData = serde_json::from_str(&content)?;

        Ok(progress.url == url)
    }

    pub fn clear(&self) -> Result<()> {
        if self.progress_file.exists() {
            fs::remove_file(&self.progress_file)?;
        }
        Ok(())
    }

    pub fn get_completed_tracks(&self) -> Result<Vec<String>> {
        if !self.progress_file.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.progress_file)?;
        let progress: ProgressData = serde_json::from_str(&content)?;

        Ok(progress.completed_tracks)
    }

    fn load_or_create(&self) -> Result<ProgressData> {
        if self.progress_file.exists() {
            let content = fs::read_to_string(&self.progress_file)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(ProgressData {
                url: String::new(),
                completed_tracks: Vec::new(),
            })
        }
    }

    fn save(&self, progress: &ProgressData) -> Result<()> {
        let content = serde_json::to_string_pretty(&progress)?;
        fs::write(&self.progress_file, content)?;
        Ok(())
    }
}

impl Default for DownloadProgress {
    fn default() -> Self {
        Self::new()
    }
}