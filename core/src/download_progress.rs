use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct DownloadKey {
    url: String,
    count_in: bool,
    transpose: i8,
}

#[derive(Debug, Serialize, Deserialize)]
struct DownloadRecord {
    #[serde(flatten)]
    key: DownloadKey,
    completed_tracks: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProgressData {
    active_download: Option<DownloadKey>,
    downloads: Vec<DownloadRecord>,
}

#[derive(Debug, Deserialize)]
struct LegacyProgressData {
    url: String,
    #[serde(default)]
    count_in: bool,
    #[serde(default)]
    transpose: i8,
    completed_tracks: Vec<String>,
}

pub struct DownloadProgress {
    progress_file: PathBuf,
}

impl DownloadProgress {
    pub fn new() -> Self {
        Self::new_with_path(None)
    }

    pub fn new_with_path(_download_path: Option<&str>) -> Self {
        Self {
            progress_file: default_progress_file(),
        }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.progress_file
    }

    #[cfg(test)]
    fn new_for_test(directory: &std::path::Path) -> Self {
        Self {
            progress_file: directory.join("download-progress.json"),
        }
    }

    pub fn log_resume_status(&self) -> Result<()> {
        tracing::info!("Using download progress file: {}", self.progress_file.display());
        let completed = self.get_completed_tracks()?;
        if !completed.is_empty() {
            tracing::info!(
                "Found {} completed track(s); they will be skipped: {}",
                completed.len(),
                completed.join(", ")
            );
        }
        Ok(())
    }

    pub fn is_track_downloaded(&self, track_name: &str) -> Result<bool> {
        let progress = self.load_or_create()?;
        Ok(active_record(&progress)
            .map(|record| record.completed_tracks.iter().any(|track| track == track_name))
            .unwrap_or(false))
    }

    pub fn mark_track_downloaded(&self, track_name: &str) -> Result<()> {
        let mut progress = self.load_or_create()?;
        let Some(active) = progress.active_download.clone() else {
            return Ok(());
        };
        let record = progress
            .downloads
            .iter_mut()
            .find(|record| record.key == active)
            .expect("active download record must exist");

        if !record.completed_tracks.iter().any(|track| track == track_name) {
            record.completed_tracks.push(track_name.to_string());
            self.save(&progress)?;
        }
        Ok(())
    }

    pub fn set_download(&self, url: &str, count_in: bool, transpose: i8) -> Result<()> {
        let mut progress = self.load_or_create()?;
        let key = DownloadKey {
            url: url.to_string(),
            count_in,
            transpose,
        };
        if !progress.downloads.iter().any(|record| record.key == key) {
            progress.downloads.push(DownloadRecord {
                key: key.clone(),
                completed_tracks: Vec::new(),
            });
        }
        progress.active_download = Some(key);
        self.save(&progress)
    }

    pub fn is_same_download(&self, url: &str, count_in: bool, transpose: i8) -> Result<bool> {
        if !self.progress_file.exists() {
            return Ok(false);
        }
        let progress = self.load_or_create()?;
        Ok(progress.active_download.as_ref().is_some_and(|active| {
            active.url == url && active.count_in == count_in && active.transpose == transpose
        }))
    }

    pub fn tracked_song_urls(&self) -> Result<Vec<String>> {
        if !self.progress_file.exists() {
            return Ok(Vec::new());
        }
        let progress = self.load_or_create()?;
        let mut urls = progress
            .downloads
            .into_iter()
            .map(|record| record.key.url)
            .collect::<Vec<_>>();
        urls.sort();
        urls.dedup();
        Ok(urls)
    }

    /// Removes every saved mix variant for a song URL.
    pub fn clear_song(&self, url: &str) -> Result<bool> {
        if !self.progress_file.exists() {
            return Ok(false);
        }
        let mut progress = self.load_or_create()?;
        let previous_count = progress.downloads.len();
        progress.downloads.retain(|record| record.key.url != url);
        let removed = progress.downloads.len() != previous_count;
        if progress
            .active_download
            .as_ref()
            .is_some_and(|active| active.url == url)
        {
            progress.active_download = None;
        }
        if removed {
            self.save(&progress)?;
        }
        Ok(removed)
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
        let progress = self.load_or_create()?;
        Ok(active_record(&progress)
            .map(|record| record.completed_tracks.clone())
            .unwrap_or_default())
    }

    fn load_or_create(&self) -> Result<ProgressData> {
        if !self.progress_file.exists() {
            return Ok(ProgressData::default());
        }

        let content = fs::read_to_string(&self.progress_file)?;
        let value: serde_json::Value = serde_json::from_str(&content)?;
        if value.get("downloads").is_some() {
            return Ok(serde_json::from_value(value)?);
        }

        let legacy: LegacyProgressData = serde_json::from_value(value)?;
        if legacy.url.is_empty() {
            return Ok(ProgressData::default());
        }
        let key = DownloadKey {
            url: legacy.url,
            count_in: legacy.count_in,
            transpose: legacy.transpose,
        };
        Ok(ProgressData {
            active_download: Some(key.clone()),
            downloads: vec![DownloadRecord {
                key,
                completed_tracks: legacy.completed_tracks,
            }],
        })
    }

    fn save(&self, progress: &ProgressData) -> Result<()> {
        let content = serde_json::to_string_pretty(progress)?;
        if let Some(parent) = self.progress_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.progress_file, content)?;
        tracing::debug!("Saved progress to {}", self.progress_file.display());
        Ok(())
    }
}

fn active_record(progress: &ProgressData) -> Option<&DownloadRecord> {
    let active = progress.active_download.as_ref()?;
    progress.downloads.iter().find(|record| &record.key == active)
}

fn default_progress_file() -> PathBuf {
    static PROGRESS_FILE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    PROGRESS_FILE
        .get_or_init(|| {
            let state_directory = dirs::state_dir()
                .or_else(dirs::data_local_dir)
                .or_else(|| dirs::home_dir().map(|home| home.join(".local/state")))
                .unwrap_or_else(|| PathBuf::from("."))
                .join("kv-downloader");
            let progress_file = state_directory.join("download-progress.json");

            if !progress_file.exists() {
                migrate_legacy_progress_file(&progress_file);
            }

            progress_file
        })
        .clone()
}

fn migrate_legacy_progress_file(progress_file: &std::path::Path) {
    let filename = ".kv_download_progress.json";
    let mut candidates = Vec::new();
    if let Ok(current_directory) = std::env::current_dir() {
        candidates.push(current_directory.join(filename));
    }
    if let Some(downloads) = dirs::home_dir().map(|home| home.join("Downloads")) {
        candidates.push(downloads.join(filename));
    }

    let Some(legacy_file) = candidates.into_iter().find(|path| path.is_file()) else {
        return;
    };
    let Some(parent) = progress_file.parent() else {
        return;
    };
    if let Err(err) = fs::create_dir_all(parent) {
        tracing::warn!("Could not create progress directory {}: {}", parent.display(), err);
        return;
    }

    if let Err(rename_error) = fs::rename(&legacy_file, progress_file) {
        match fs::copy(&legacy_file, progress_file) {
            Ok(_) => {
                let _ = fs::remove_file(&legacy_file);
            }
            Err(copy_error) => tracing::warn!(
                "Could not migrate progress file to {}: {}; {}",
                progress_file.display(),
                rename_error,
                copy_error
            ),
        }
    } else {
        tracing::info!("Migrated download progress to {}", progress_file.display());
    }
}

impl Default for DownloadProgress {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::DownloadProgress;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

    fn temporary_directory() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = NEXT_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "kv-downloader-progress-{}-{unique}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn retains_completed_tracks_for_the_same_mix() {
        let directory = temporary_directory();
        let progress = DownloadProgress::new_for_test(&directory);
        progress.set_download("song-one", true, -1).unwrap();
        progress.mark_track_downloaded("Drum Kit").unwrap();
        progress.set_download("song-one", true, -1).unwrap();
        assert!(progress.is_track_downloaded("Drum Kit").unwrap());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn retains_separate_progress_for_multiple_songs_and_mix_options() {
        let directory = temporary_directory();
        let progress = DownloadProgress::new_for_test(&directory);
        progress.set_download("song-one", false, 0).unwrap();
        progress.mark_track_downloaded("Drum Kit").unwrap();
        progress.set_download("song-two", false, 0).unwrap();
        progress.mark_track_downloaded("Bass").unwrap();
        progress.set_download("song-one", true, 0).unwrap();
        assert!(!progress.is_track_downloaded("Drum Kit").unwrap());
        progress.set_download("song-one", false, 0).unwrap();
        assert!(progress.is_track_downloaded("Drum Kit").unwrap());
        progress.set_download("song-two", false, 0).unwrap();
        assert!(progress.is_track_downloaded("Bass").unwrap());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn clears_all_mix_variants_for_only_the_selected_song() {
        let directory = temporary_directory();
        let progress = DownloadProgress::new_for_test(&directory);
        progress.set_download("song-one", false, 0).unwrap();
        progress.mark_track_downloaded("Drum Kit").unwrap();
        progress.set_download("song-one", true, 0).unwrap();
        progress.mark_track_downloaded("Bass").unwrap();
        progress.set_download("song-two", false, 0).unwrap();
        progress.mark_track_downloaded("Piano").unwrap();

        assert!(progress.clear_song("song-one").unwrap());
        progress.set_download("song-one", false, 0).unwrap();
        assert!(!progress.is_track_downloaded("Drum Kit").unwrap());
        progress.set_download("song-two", false, 0).unwrap();
        assert!(progress.is_track_downloaded("Piano").unwrap());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reads_and_upgrades_legacy_single_song_format() {
        let directory = temporary_directory();
        let path = directory.join("download-progress.json");
        fs::write(
            &path,
            r#"{"url":"song-one","count_in":false,"transpose":0,"completed_tracks":["Bass"]}"#,
        )
        .unwrap();
        let progress = DownloadProgress::new_for_test(&directory);
        assert!(progress.is_track_downloaded("Bass").unwrap());
        progress.mark_track_downloaded("Piano").unwrap();
        assert!(progress.is_track_downloaded("Piano").unwrap());
        fs::remove_dir_all(directory).unwrap();
    }
}
