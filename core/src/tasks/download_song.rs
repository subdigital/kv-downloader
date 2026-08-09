use crate::driver::{Config, Driver};
use crate::download_progress::DownloadProgress;
use crate::keystore::Keystore;

use anyhow::{anyhow, Result};
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use headless_chrome::protocol::cdp::types::Event;
use headless_chrome::protocol::cdp::Page::StartScreencastFormatOption;
use headless_chrome::{Element, Tab};
use headless_chrome::browser::tab::RequestPausedDecision;
use headless_chrome::protocol::cdp::Fetch::events::RequestPausedEvent;
use reqwest::blocking::Client;
use std::fmt::Display;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::{env, error::Error, thread::sleep, time::Duration, time::SystemTime};

/// Maximum time to wait for a download to complete (in seconds)
const DOWNLOAD_COMPLETION_TIMEOUT_SECS: u64 = 300; // 5 minutes

#[derive(Default)]
pub struct DownloadOptions {
    pub count_in: bool,
    pub transpose: i8,
    pub selected_tracks: Option<Vec<String>>,
    pub force_restart: bool,
}

#[derive(Debug)]
pub enum DownloadError {
    NotPurchased,
    NotASongPage,
    HumanVerificationRequired,
}

impl Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPurchased => f.write_str("This track has not been purchased"),
            Self::NotASongPage => f.write_str("This doesn't look like a song page. Check the url."),
            Self::HumanVerificationRequired => f.write_str("The headless browser was detected as a bot and is being presented with a 'Verify you are human' step. Try running without --headless.")
        }
    }
}
impl Error for DownloadError {}

impl Driver {
    pub fn download_song(&self, url: &str, options: DownloadOptions) -> Result<()> {
        if options.force_restart {
            self.progress.clear()?;
        }
        self.progress
            .set_download(url, options.count_in, options.transpose)?;
        self.progress.log_resume_status()?;

        let tab = self.browser.new_tab()?;
        self.minimize_tab(&tab);
        tab.set_default_timeout(Duration::from_secs(30));

        self.enable_network_capture(&tab)?;

        tab.navigate_to(url)?.wait_until_navigated()?;

        if !self.is_a_song_page(&tab) {
            tab.stop_screencast()?;

            if self.is_verify_you_are_human_page(&tab) {
                return Err(anyhow!(DownloadError::HumanVerificationRequired));
            } else {
                return Err(anyhow!(DownloadError::NotASongPage));
            }
        }

        if !self.is_downloadable(&tab) {
            tab.stop_screencast()?;
            return Err(anyhow!(DownloadError::NotPurchased));
        }

        if options.count_in {
            let el = tab
                .wait_for_element_with_custom_timeout("input#precount", Duration::from_secs(15))?;
            if !el.is_checked() {
                el.click()?;
            }
        }

        self.adjust_pitch(options.transpose, &tab)?;

        self.solo_and_download_tracks(&tab, options.selected_tracks.as_deref())?;

        tab.stop_screencast()?;

        Ok(())
    }

    fn solo_and_download_tracks(
        &self,
        tab: &Tab,
        selected_tracks: Option<&[String]>,
    ) -> Result<()> {
        let solo_button_sel = ".track__controls.track__solo";
        let solo_buttons = tab.find_elements(solo_button_sel)?;
        let download_button = tab.find_element("a.download")?;
        let track_names = Driver::extract_track_names(tab)?
            .into_iter()
            .map(|name| normalize_track_name(&name))
            .collect::<Vec<_>>();
        let selected = selected_tracks.map(|tracks| {
            tracks
                .iter()
                .map(|t| normalize_track_name(t))
                .collect::<std::collections::HashSet<String>>()
        });

        tab.enable_debugger()?;
        sleep(Duration::from_secs(2));

        let mut failed_tracks = Vec::new();

        for (index, solo_btn) in solo_buttons.iter().enumerate() {
            let track_name = track_names[index].clone();

            if let Some(selected) = &selected {
                if !selected.contains(&track_name) {
                    tracing::debug!("Skipping track '{}' (not selected)", track_name);
                    continue;
                }
            }

            // Check if track was already downloaded
            if self.progress.is_track_downloaded(&track_name)? {
                tracing::info!(
                    "Skipping track {} '{}' (recorded as completed in {})",
                    index + 1,
                    track_name,
                    self.progress.path().display()
                );
                continue;
            }

            tracing::info!("Processing track {} '{}'", index + 1, track_name);

            // Try downloading with retries
            let mut attempts = 0;
            let max_attempts = 3;
            let mut download_successful = false;

            while attempts < max_attempts && !download_successful {
                attempts += 1;

                match self.download_single_track(
                    tab,
                    solo_btn,
                    &download_button,
                    &track_name,
                    attempts,
                ) {
                    Ok(_) => {
                        download_successful = true;
                        tracing::info!("- '{}' complete!", track_name);
                        self.progress.mark_track_downloaded(&track_name)?;
                    }
                    Err(e) => {
                        tracing::warn!("Attempt {} failed for '{}': {}", attempts, track_name, e);
                        if attempts < max_attempts {
                            let wait_time = Duration::from_secs(5 * attempts as u64);
                            tracing::info!("Waiting {:?} before retry...", wait_time);
                            sleep(wait_time);
                        }
                    }
                }
            }

            if !download_successful {
                failed_tracks.push(track_name.clone());
                tracing::error!(
                    "Failed to download '{}' after {} attempts",
                    track_name,
                    max_attempts
                );
            }
        }

        if failed_tracks.is_empty() {
            tracing::info!(
                "Done! All tracks downloaded successfully: {}\n - ",
                track_names.join("\n - ")
            );
            tracing::info!("Progress retained so completed tracks are skipped on future runs");
        } else {
            tracing::warn!(
                "Download completed with {} failures. Failed tracks:\n - {}",
                failed_tracks.len(),
                failed_tracks.join("\n - ")
            );
            tracing::info!("Progress saved. Run the command again to retry failed tracks.");
            return Err(anyhow!("{} tracks failed to download", failed_tracks.len()));
        }

        Ok(())
    }

    fn download_single_track(
        &self,
        tab: &Tab,
        solo_btn: &Element,
        download_button: &Element,
        track_name: &str,
        attempt: u32,
    ) -> Result<()> {
        if attempt > 1 {
            tracing::info!("Attempt {} for track '{}'", attempt, track_name);
        }

        solo_btn.scroll_into_view()?;
        sleep(Duration::from_millis(500));
        solo_btn.click()?;
        sleep(Duration::from_millis(500));

        tracing::info!("- starting download...");
        download_button.scroll_into_view()?;
        sleep(Duration::from_millis(500));
        download_button.click()?;
        sleep(Duration::from_millis(500));

        tracing::info!("- waiting for download modal...");

        // Increase timeout for retries
        let timeout = Duration::from_secs(60 + (attempt as u64 - 1) * 30);
        tab.wait_for_element_with_custom_timeout(".begin-download", timeout)
            .map_err(|_| anyhow!("Timed out waiting for download modal after {:?}", timeout))?;

        // Wait a bit for the modal to be fully rendered
        sleep(Duration::from_secs(1));

        // Extract the filename from the download link before closing the modal
        let filename = self.extract_download_filename(tab)?;
        tracing::debug!("Expected download filename: {}", filename);

        // Try to find and close the modal
        match tab.find_element("button.js-modal-close") {
            Ok(close_btn) => {
                close_btn.click()?;
            }
            Err(_) => {
                tracing::warn!("Could not find modal close button, proceeding anyway");
            }
        }
        sleep(Duration::from_secs(4));

        // Wait for the download to complete
        self.wait_for_download_completion(&filename)?;

        Ok(())
    }

    fn extract_download_filename(&self, tab: &Tab) -> Result<String> {
        // Try to find the download link in the modal
        let download_link = tab.find_element("div.begin-download a")?;

        // Get the href attribute which should contain the filename
        let href = download_link
            .get_attribute_value("href")?
            .ok_or_else(|| anyhow!("Download link has no href attribute"))?;

        // Extract filename from the URL
        let filename = href
            .split('/')
            .last()
            .ok_or_else(|| anyhow!("Could not extract filename from URL"))?
            .to_string();

        // Decode URL-encoded characters
        let decoded = urlencoding::decode(&filename)
            .map_err(|e| anyhow!("Failed to decode filename: {}", e))?
            .to_string();

        Ok(decoded)
    }

    fn wait_for_download_completion(&self, expected_filename: &str) -> Result<()> {
        let download_path = match &self.config.download_path {
            Some(path) => PathBuf::from(path),
            None => Self::get_default_download_dir()?,
        };

        tracing::info!("- waiting for download to complete...");
        tracing::debug!("Monitoring directory: {}", download_path.display());

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(DOWNLOAD_COMPLETION_TIMEOUT_SECS);

        // The .crdownload file will have the same name as the final file with .crdownload appended
        let crdownload_filename = format!("{}.crdownload", expected_filename);

        // Poll for the specific .crdownload file
        loop {
            if start.elapsed() > timeout {
                return Err(anyhow!("Download did not complete within {:?}", timeout));
            }

            // Check if the specific .crdownload file exists
            match std::fs::read_dir(&download_path) {
                Ok(entries) => {
                    let has_crdownload = entries
                        .filter_map(|e| e.ok())
                        .any(|entry| entry.file_name().to_string_lossy() == crdownload_filename);

                    if !has_crdownload {
                        // Verify the final file actually exists
                        let final_path = download_path.join(expected_filename);
                        if final_path.exists() {
                            tracing::info!("- download complete");
                            return Ok(());
                        }
                        // If .crdownload is gone but final file doesn't exist yet, keep waiting
                    }
                }
                Err(e) => {
                    tracing::warn!("Could not read download directory: {}", e);
                    return Err(anyhow!("Failed to read download directory: {}", e));
                }
            }

            // Wait a bit before checking again
            sleep(Duration::from_millis(500));
        }
    }

    fn get_default_download_dir() -> Result<PathBuf> {
        // Get the user's home directory
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;

        // Default download directory varies by OS
        #[cfg(target_os = "macos")]
        let download_dir = home.join("Downloads");

        #[cfg(target_os = "linux")]
        let download_dir = home.join("Downloads");

        #[cfg(target_os = "windows")]
        let download_dir = home.join("Downloads");

        if !download_dir.exists() {
            return Err(anyhow!(
                "Default download directory does not exist: {}",
                download_dir.display()
            ));
        }

        Ok(download_dir)
    }

    fn enable_network_capture(&self, tab: &Tab) -> Result<()> {
        if env::var("KV_CAPTURE_NETWORK").ok().as_deref() != Some("1") {
            return Ok(());
        }

        let log_dir = match &self.config.download_path {
            Some(path) => PathBuf::from(path),
            None => Self::get_default_download_dir()?,
        };
        let log_path = log_dir.join("kv_network.log");

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let file = Arc::new(Mutex::new(file));

        if let Ok(mut handle) = file.lock() {
            let _ = writeln!(
                handle,
                "\n=== capture started: {:?} ===",
                SystemTime::now()
            );
        }

        tab.enable_fetch(None, None)?;

        let request_file = Arc::clone(&file);
        tab.enable_request_interception(Arc::new(
            move |_transport, _session, event: RequestPausedEvent| {
            if let Ok(mut handle) = request_file.lock() {
                let _ = writeln!(handle, "[request] {:?}", event.params);
            }
            RequestPausedDecision::Continue(None)
        },
        ))?;

        let response_file = Arc::clone(&file);
        tab.register_response_handling("kv_capture", Box::new(move |params, _get_body| {
            if let Ok(mut handle) = response_file.lock() {
                let _ = writeln!(handle, "[response] {:?}", params);
            }
        }))?;

        tracing::info!(
            "Network capture enabled (KV_CAPTURE_NETWORK=1). Logging to {}",
            log_path.display()
        );

        Ok(())
    }

    pub fn extract_track_names(tab: &Tab) -> Result<Vec<String>> {
        let track_names = tab.find_elements(".mixer .track .track__caption")?;
        let mut names: Vec<String> = vec![];
        for el in track_names {
            // the name may contain other child nodes, so we'll execute a js function
            // to just grab the last child, which is the text.
            let name: String = el
                .call_js_fn(
                    r#"
                    function get_name() {
                        return this.lastChild.nodeValue.trim();
                    }
                    "#,
                    vec![],
                    true,
                )?
                .value
                // remove quotes & new lines from the extracted text
                .map(|v| v.to_string().replace("\\n", " ").replace('"', ""))
                .unwrap_or(String::new());
            names.push(name);
        }

        Ok(names)
    }

    fn is_a_song_page(&self, tab: &Tab) -> bool {
        let has_mixer = tab.find_element("div.mixer").is_ok();
        let has_download_button = tab.find_element("a.download").is_ok();

        has_mixer && has_download_button
    }

    fn is_verify_you_are_human_page(&self, tab: &Tab) -> bool {
        tab.get_title()
            .ok()
            .unwrap_or_default()
            .contains("Suspicious activity has been detected")
    }

    fn is_downloadable(&self, tab: &Tab) -> bool {
        // if the download button also has the addtocart class, then this hasn't been purchased
        let el = tab.find_element("a.download.addtocart").ok();
        el.is_none()
    }

    fn adjust_pitch(&self, desired_pitch: i8, tab: &Tab) -> Result<()> {
        // pitch is remembered per-son on your account, so this logic cannot be deterministic. Instead
        // we''l try to infer the direction we need to go based on what the pitch is currently set to.
        let pitch_label = tab
            .find_element("span.pitch__value")
            .expect("can't find pitch value");
        let pitch_up_btn = tab
            .find_element("div.pitch button.btn--pitch[title='Key up' i]")
            .expect("can't find pitch up button");
        let pitch_down_btn = tab
            .find_element("div.pitch button.btn--pitch[title='Key down' i]")
            .expect("can't find pitch down button");

        pitch_up_btn.focus()?;

        let current_pitch: i8 = pitch_label.get_inner_text()?.parse()?;
        let diff = desired_pitch - current_pitch;
        if diff == 0 {
            return Ok(());
        }
        tracing::info!(
            "Setting pitch to {} (currently: {})",
            desired_pitch,
            current_pitch
        );

        let button = if diff > 0 {
            pitch_up_btn
        } else {
            pitch_down_btn
        };

        let mut iterations_allowed = 10;
        loop {
            assert!(
                iterations_allowed > 0,
                "failed to set pitch, breaking to avoid infinite loop"
            );
            iterations_allowed -= 1;

            tracing::debug!("Pitching tracks...");
            button.click().expect("couldn't click pitch button");
            sleep(Duration::from_millis(100));

            let new_pitch: i8 = pitch_label.get_inner_text()?.parse()?;
            tracing::debug!("Pitching is now {}, target: {}", new_pitch, desired_pitch);
            sleep(Duration::from_millis(100));

            if new_pitch == desired_pitch {
                break;
            }
        }

        // need to reload the song after pitching
        tracing::info!("Reloading tracks after pitching...");
        tab.find_element("a#pitch-link")
            .expect("can't find pitch link")
            .click()?;

        sleep(Duration::from_secs(4));

        Ok(())
    }

    #[allow(dead_code)]
    fn print_source_html(&self, tab: &Tab) {
        let source_obj = tab
            .evaluate("document.documentElement.outerHTML", true)
            .ok()
            .unwrap();
        let source_html = source_obj.value.unwrap().as_str().unwrap().to_string();
        println!("{}", source_html);
    }

    #[allow(dead_code)]
    fn record_screencast(&self, tab: &Tab) -> Result<()> {
        tab.add_event_listener(Arc::new(|event: &Event| match event {
            Event::PageScreencastFrame(frame_event) => {
                let bytes = BASE64_STANDARD
                    .decode(frame_event.params.data.clone())
                    .unwrap();
                let ts = frame_event.params.metadata.timestamp.unwrap();
                std::fs::write(format!("screencast-{}.jpg", ts), &bytes).unwrap();
            }
            _ => {}
        }))?;

        tab.start_screencast(
            Some(StartScreencastFormatOption::Jpeg),
            Some(80),
            Some(1280),
            Some(720),
            Some(4),
        )?;

        Ok(())
    }
}

pub fn download_song_http(
    url: &str,
    options: DownloadOptions,
    download_path: Option<String>,
    domain: &str,
) -> Result<()> {
    let progress = DownloadProgress::new_with_path(download_path.as_deref());
    if options.force_restart {
        tracing::info!("Force restart requested, clearing previous progress");
        progress.clear()?;
    }
    progress.set_download(url, options.count_in, options.transpose)?;
    progress.log_resume_status()?;

    let cookie = Keystore::get_auth_cookie_value()
        .map_err(|_| anyhow!("Missing session cookie. Run `kv-downloader auth` first."))?;
    let mut cookie_header = format!("karaoke-version={}", cookie);

    let client = Client::builder()
        .user_agent("kv-downloader-http")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(DOWNLOAD_COMPLETION_TIMEOUT_SECS))
        .build()?;

    let mut song_page = client
        .get(url)
        .timeout(Duration::from_secs(30))
        .header("Cookie", &cookie_header)
        .send()?
        .text()?;

    write_http_debug_page(download_path.as_deref(), &song_page);

    let mut tried_browser_verification = false;
    if is_browser_challenge(&song_page) {
        tracing::warn!("Browser verification challenge detected. Opening a browser window to let you solve it.");
        cookie_header =
            resolve_browser_challenge(url, domain, download_path.as_deref(), &cookie)?;
        tried_browser_verification = true;
        song_page = client
            .get(url)
            .timeout(Duration::from_secs(30))
            .header("Cookie", &cookie_header)
            .send()?
            .text()?;

        if is_browser_challenge(&song_page) {
            return Err(anyhow!(
                "Browser verification challenge is still active after browser verification"
            ));
        }
    }

    write_http_debug_page(download_path.as_deref(), &song_page);
    let song_meta = match SongMeta::parse(&song_page) {
        Ok(meta) => meta,
        Err(err) if !tried_browser_verification => {
            write_http_debug_page(download_path.as_deref(), &song_page);
            tracing::warn!(
                "Song page did not contain expected mixer metadata ({}). Opening a browser window to refresh verification and session state.",
                err
            );
            cookie_header =
                resolve_browser_challenge(url, domain, download_path.as_deref(), &cookie)?;
            song_page = client
                .get(url)
                .timeout(Duration::from_secs(30))
                .header("Cookie", &cookie_header)
                .send()?
                .text()?;
            SongMeta::parse(&song_page).map_err(|retry_err| {
                write_http_debug_page(download_path.as_deref(), &song_page);
                anyhow!(
                    "Failed to parse song page after browser verification: {}",
                    retry_err
                )
            })?
        }
        Err(err) => {
            write_http_debug_page(download_path.as_deref(), &song_page);
            return Err(anyhow!("Failed to parse song page: {}", err));
        }
    };

    let selected_indices = match options.selected_tracks {
        Some(tracks) => song_meta.indices_for_tracks(&tracks)?,
        None => song_meta.default_track_indices(),
    };

    let download_dir = match &download_path {
        Some(path) => PathBuf::from(path),
        None => Driver::get_default_download_dir()?,
    };

    let mut failed_tracks = Vec::new();
    let mut downloaded_files: Vec<PathBuf> = Vec::new();

    for (index, track_name) in song_meta.track_names.iter().enumerate() {
        let level_index = song_meta.track_level_indices[index];

        if !selected_indices.contains(&level_index) {
            continue;
        }

        if progress.is_track_downloaded(track_name)? {
            tracing::info!(
                "Skipping '{}' (recorded as completed in {})",
                track_name,
                progress.path().display()
            );
            continue;
        }

        tracing::info!("Processing track {} '{}'", level_index, track_name);

        let mut completed = false;
        for attempt in 1..=3 {
            tracing::info!("Attempt {}/3 for '{}'", attempt, track_name);
            match download_http_track(
                &client,
                &cookie_header,
                domain,
                &song_meta,
                track_name,
                level_index,
                options.count_in,
                options.transpose,
                &download_dir,
            ) {
                Ok((decoded, file_path)) => {
                    if let Some(previous) = downloaded_files
                        .iter()
                        .find(|previous| files_are_identical(previous, &file_path).unwrap_or(false))
                    {
                        tracing::warn!(
                            "Rejected '{}' because its contents are identical to '{}'; the server likely returned a stale mix",
                            decoded,
                            previous.display()
                        );
                        let _ = std::fs::remove_file(&file_path);
                        if attempt < 3 {
                            tracing::info!("Retrying '{}' in 5 seconds", track_name);
                            sleep(Duration::from_secs(5));
                        }
                        continue;
                    }

                    tracing::info!("Downloaded '{}'", decoded);
                    progress.mark_track_downloaded(track_name)?;
                    downloaded_files.push(file_path);
                    completed = true;
                    break;
                }
                Err(err) => {
                    tracing::warn!(
                        "Attempt {}/3 failed for '{}': {}",
                        attempt,
                        track_name,
                        err
                    );
                    if attempt < 3 {
                        tracing::info!("Retrying '{}' in 5 seconds", track_name);
                        sleep(Duration::from_secs(5));
                    }
                }
            }
        }

        if !completed {
            tracing::error!("Giving up on '{}' after 3 attempts; continuing", track_name);
            failed_tracks.push(track_name.clone());
        }
    }

    if failed_tracks.is_empty() {
        tracing::info!("Download complete; progress retained for future runs");
        Ok(())
    } else {
        Err(anyhow!(
            "{} track(s) failed after retries: {}. Completed tracks were saved and will be skipped next time.",
            failed_tracks.len(),
            failed_tracks.join(", ")
        ))
    }
}

fn download_http_track(
    client: &Client,
    cookie_header: &str,
    domain: &str,
    song_meta: &SongMeta,
    track_name: &str,
    level_index: usize,
    count_in: bool,
    transpose: i8,
    download_dir: &std::path::Path,
) -> Result<(String, PathBuf)> {
    let trackslevels = song_meta.build_trackslevels_solo(level_index)?;
    if env::var("KV_HTTP_DEBUG").ok().as_deref() == Some("1") {
        tracing::info!(
            "HTTP mix params: track='{}' index={} trackslevels={} pannings={}",
            track_name,
            level_index,
            trackslevels,
            song_meta.pannings
        );
    }

    let params = vec![
        ("method", "ajax".to_string()),
        ("famid", song_meta.famid.to_string()),
        ("precount", if count_in { "1" } else { "0" }.to_string()),
        ("trackslevels", trackslevels),
        ("pannings", song_meta.pannings.clone()),
        ("bkac", song_meta.bkac.clone()),
        ("s", song_meta.song_id.to_string()),
        ("prodid", song_meta.prod_id.to_string()),
        ("pitch", transpose.to_string()),
    ];

    let basket_url = format!("https://{domain}/basket.php");
    tracing::info!("Requesting mix build for '{}'", track_name);
    client
        .get(&basket_url)
        .timeout(Duration::from_secs(30))
        .query(&params)
        .header("Cookie", cookie_header)
        .header("X-Requested-With", "XMLHttpRequest")
        .send()?
        .error_for_status()?;

    // A fast basket response can leave the previous completed mix visible briefly.
    // Give the server time to register this track before checking for its file.
    sleep(Duration::from_secs(1));

    let begin_url = format!(
        "https://{domain}/my/begin_download.html?id={}&famid={}",
        song_meta.prod_id, song_meta.famid
    );
    tracing::info!("Waiting for '{}' to be prepared", track_name);
    let file_url = poll_for_file_url(client, cookie_header, &begin_url)?;
    let filename = file_url
        .split('/')
        .last()
        .ok_or_else(|| anyhow!("Missing filename in download url"))?;
    let decoded = urlencoding::decode(filename)
        .map_err(|e| anyhow!("Failed to decode filename: {}", e))?
        .to_string();

    let file_path = download_dir.join(&decoded);
    tracing::info!("Downloading '{}'", decoded);
    download_file(client, &file_url, &file_path)?;
    Ok((decoded, file_path))
}

fn files_are_identical(first: &std::path::Path, second: &std::path::Path) -> Result<bool> {
    use std::io::Read;

    if std::fs::metadata(first)?.len() != std::fs::metadata(second)?.len() {
        return Ok(false);
    }

    let mut first = std::fs::File::open(first)?;
    let mut second = std::fs::File::open(second)?;
    let mut first_buffer = [0u8; 64 * 1024];
    let mut second_buffer = [0u8; 64 * 1024];
    loop {
        let first_read = first.read(&mut first_buffer)?;
        let second_read = second.read(&mut second_buffer)?;
        if first_read != second_read || first_buffer[..first_read] != second_buffer[..second_read] {
            return Ok(false);
        }
        if first_read == 0 {
            return Ok(true);
        }
    }
}

fn write_http_debug_page(download_path: Option<&str>, html: &str) {
    if env::var("KV_HTTP_DEBUG").ok().as_deref() != Some("1") {
        return;
    }

    let debug_path = download_path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kv_http_song.html");
    if let Err(err) = std::fs::write(&debug_path, html) {
        tracing::warn!("Failed to write debug html to {}: {}", debug_path.display(), err);
    } else {
        tracing::warn!("Wrote debug html to {}", debug_path.display());
    }
}

fn is_browser_challenge(html: &str) -> bool {
    let normalized = html.to_lowercase();
    normalized.contains("cdn-cgi/challenge-platform")
        || normalized.contains("cf-challenge")
        || normalized.contains("cf-turnstile")
        || normalized.contains("__cf_chl")
        || normalized.contains("challenge-platform")
        || normalized.contains("verify you are human")
        || normalized.contains("just a moment")
        || normalized.contains("fastly")
        || normalized.contains("<title>client challenge</title>")
        || normalized.contains("/_fs-ch-")
        || normalized.contains("fastly error")
        || normalized.contains("guru meditation")
        || normalized.contains("varnish cache server")
}

fn resolve_browser_challenge(
    url: &str,
    domain: &str,
    download_path: Option<&str>,
    session_cookie_value: &str,
) -> Result<String> {
    let config = Config {
        domain: domain.to_string(),
        headless: false,
        download_path: download_path.map(ToString::to_string),
        idle_browser_timeout: Duration::from_secs(DOWNLOAD_COMPLETION_TIMEOUT_SECS),
    };
    let driver = Driver::new(config);
    let tab = driver.browser.new_tab()?;

    tab.navigate_to(&format!("https://{domain}"))?
        .wait_until_navigated()?;
    tab.set_cookies(vec![headless_chrome::protocol::cdp::Network::CookieParam {
        name: "karaoke-version".to_string(),
        value: session_cookie_value.to_string(),
        url: Some(format!("https://{domain}")),
        domain: None,
        secure: None,
        http_only: None,
        same_site: None,
        path: None,
        expires: None,
        priority: None,
        same_party: None,
        source_scheme: None,
        source_port: None,
        partition_key: None,
    }])?;

    tab.navigate_to(url)?.wait_until_navigated()?;
    tracing::warn!(
        "Please complete browser verification and sign in if prompted. Keep the window open until the song mixer appears. Waiting up to {} seconds...",
        DOWNLOAD_COMPLETION_TIMEOUT_SECS
    );

    let started = std::time::Instant::now();
    let mut last_log = std::time::Instant::now();
    loop {
        let html = tab.get_content()?;
        if SongMeta::parse(&html).is_ok() {
            let cookies = tab.get_cookies()?;
            if let Some(session_cookie) = cookies.iter().find(|c| c.name == "karaoke-version") {
                if session_cookie.value != session_cookie_value
                    && env::var("KV_SESSION_COOKIE").is_err()
                {
                    Keystore::set_auth_cookie(session_cookie)?;
                }
            }

            let cookie_header = cookies
                .iter()
                .filter(|cookie| domain.ends_with(cookie.domain.trim_start_matches('.')))
                .map(|cookie| format!("{}={}", cookie.name, cookie.value))
                .collect::<Vec<_>>()
                .join("; ");
            if cookie_header.is_empty() {
                return Err(anyhow!("Browser verification completed without any site cookies"));
            }

            tracing::info!("Song mixer loaded; continuing download with browser session");
            return Ok(cookie_header);
        }

        if started.elapsed() > Duration::from_secs(DOWNLOAD_COMPLETION_TIMEOUT_SECS) {
            return Err(anyhow!("Timed out waiting for browser verification challenge to be solved"));
        }

        if last_log.elapsed() >= Duration::from_secs(10) {
            tracing::info!("Still waiting for the song mixer to load in the browser...");
            last_log = std::time::Instant::now();
        }

        sleep(Duration::from_secs(1));
    }
}

fn normalize_track_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

struct SongMeta {
    track_names: Vec<String>,
    track_level_indices: Vec<usize>,
    track_index_map: std::collections::HashMap<String, usize>,
    levels: Vec<u32>,
    pannings: String,
    song_id: u32,
    prod_id: u32,
    bkac: String,
    famid: u32,
}

impl SongMeta {
    fn parse(html: &str) -> Result<Self> {
        let track_names_raw = parse_json_array(html, "mixer.setTracksDescription(")
            .ok_or_else(|| anyhow!("Missing track list"))?;

        let levels_raw = parse_string_argument(html, "mixer.setLevels(\"", "\")")
            .ok_or_else(|| anyhow!("Missing levels"))?;
        let pannings = parse_string_argument(html, "mixer.setPannings(\"", "\")")
            .ok_or_else(|| anyhow!("Missing pannings"))?;

        let levels = parse_indexed_values(&levels_raw)?;

        let song_id = parse_number(html, "mixer.parameters.s =")
            .ok_or_else(|| anyhow!("Missing song id"))?;
        let prod_id = parse_number(html, "mixer.parameters.prodid =")
            .ok_or_else(|| anyhow!("Missing prod id"))?;
        let bkac = parse_string_argument(html, "mixer.parameters.bkac = \"", "\"")
            .ok_or_else(|| anyhow!("Missing bkac"))?;
        let uri = parse_string_argument(html, "mixer.uri = '", "'")
            .ok_or_else(|| anyhow!("Missing begin download uri"))?;
        let famid = parse_query_value(&uri, "famid")
            .ok_or_else(|| anyhow!("Missing famid"))?;

        let mut track_names = Vec::new();
        let mut track_level_indices = Vec::new();
        let mut track_index_map = std::collections::HashMap::new();
        for (pos, name) in track_names_raw.iter().enumerate() {
            let level_index = pos + 2; // index 1 is master, index 2 maps to first entry
            let normalized = normalize_metadata_track_name(name);
            if is_precount_track(&normalized) {
                continue;
            }
            track_index_map.insert(normalized.to_lowercase(), level_index);
            track_names.push(normalized);
            track_level_indices.push(level_index);
        }

        Ok(Self {
            track_names,
            track_level_indices,
            track_index_map,
            levels,
            pannings,
            song_id,
            prod_id,
            bkac,
            famid,
        })
    }

    fn indices_for_tracks(&self, tracks: &[String]) -> Result<std::collections::HashSet<usize>> {
        let mut indices = std::collections::HashSet::new();
        for track in tracks {
            if is_precount_track(track) {
                tracing::debug!(
                    "Ignoring intro count-in entry '{}' because count-in is configured separately",
                    track
                );
                continue;
            }
            let normalized = normalize_track_name(track);
            if let Some(level_index) = self.track_index_map.get(&normalized.to_lowercase()) {
                indices.insert(*level_index);
            } else {
                return Err(anyhow!(
                    "Track '{}' not found in song metadata. Available tracks: {}",
                    track,
                    self.track_names.join(", ")
                ));
            }
        }
        Ok(indices)
    }

    fn default_track_indices(&self) -> std::collections::HashSet<usize> {
        self.track_level_indices.iter().copied().collect()
    }

    fn build_trackslevels_solo(&self, level_index: usize) -> Result<String> {
        let total = self.levels.len();
        let mut values = vec![0u32; total];
        if let Some(existing) = self.levels.get(0) {
            values[0] = *existing;
        }
        if level_index == 0 || level_index > total {
            return Err(anyhow!("Selected index out of range"));
        }
        values[level_index - 1] = 100;
        Ok(encode_indexed_values(&values))
    }
}

fn parse_json_array(html: &str, marker: &str) -> Option<Vec<String>> {
    let start = html.find(marker)?;
    let after = &html[start + marker.len()..];
    let start_bracket = after.find('[')?;
    let mut depth = 0isize;
    for (idx, ch) in after[start_bracket..].char_indices() {
        if ch == '[' {
            depth += 1;
        } else if ch == ']' {
            depth -= 1;
            if depth == 0 {
                let slice = &after[start_bracket..start_bracket + idx + 1];
                return serde_json::from_str::<Vec<String>>(slice).ok();
            }
        }
    }
    None
}

fn parse_string_argument(html: &str, marker: &str, end: &str) -> Option<String> {
    let start = html.find(marker)?;
    let after = &html[start + marker.len()..];
    let end_index = after.find(end)?;
    Some(after[..end_index].to_string())
}

fn parse_number(html: &str, marker: &str) -> Option<u32> {
    let start = html.find(marker)?;
    let after = &html[start + marker.len()..];
    let digits = after
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

fn parse_query_value(uri: &str, key: &str) -> Option<u32> {
    let query = uri.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.split('=');
        let k = parts.next()?;
        let v = parts.next()?;
        if k == key {
            return v.parse().ok();
        }
    }
    None
}

fn parse_indexed_values(input: &str) -> Result<Vec<u32>> {
    let mut values: Vec<Option<u32>> = Vec::new();
    for part in input.split(',') {
        if let Some((val, idx)) = part.split_once('.') {
            let value: u32 = val.parse().unwrap_or(0);
            let index: usize = idx.parse().unwrap_or(0);
            if index == 0 {
                continue;
            }
            if values.len() < index {
                values.resize(index, None);
            }
            values[index - 1] = Some(value);
        } else {
            values.push(part.parse::<u32>().ok());
        }
    }
    let filled = values
        .into_iter()
        .map(|v| v.unwrap_or(0))
        .collect::<Vec<_>>();
    Ok(filled)
}

fn encode_indexed_values(values: &[u32]) -> String {
    let total = values.len();
    let mut parts = Vec::with_capacity(total);
    for (idx, value) in values.iter().enumerate() {
        let index = idx + 1;
        if index == 1 || index == total {
            parts.push(format!("{}", value));
        } else {
            parts.push(format!("{}.{}", value, index));
        }
    }
    parts.join(",")
}

fn normalize_metadata_track_name(value: &str) -> String {
    const CAPTION_MARKER: &str = "custom__mixer-track-caption-name";
    if let Some(marker_index) = value.find(CAPTION_MARKER) {
        let after_marker = &value[marker_index + CAPTION_MARKER.len()..];
        if let Some(content_start) = after_marker.find('>') {
            let content = &after_marker[content_start + 1..];
            if let Some(content_end) = content.find("</span>") {
                return normalize_track_name(&content[..content_end].replace("&nbsp;", " "));
            }
        }
    }
    normalize_track_name(value)
}

fn is_precount_track(value: &str) -> bool {
    let normalized = value.to_lowercase();
    normalized.contains("intro count") || normalized.contains("precount")
}

fn poll_for_file_url(client: &Client, cookie_header: &str, begin_url: &str) -> Result<String> {
    let timeout = Duration::from_secs(120);
    let start = std::time::Instant::now();
    let produced_url = if begin_url.contains('?') {
        format!("{begin_url}&produced=1&method=ajax")
    } else {
        format!("{begin_url}?produced=1&method=ajax")
    };

    let resp = client
        .get(begin_url)
        .timeout(Duration::from_secs(30))
        .header("Cookie", cookie_header)
        .header("X-Requested-With", "XMLHttpRequest")
        .send()?;

    if let Some(file_href) = resp.headers().get("x-file-href") {
        return Ok(file_href.to_str()?.to_string());
    }

    let wait_url = resp
        .headers()
        .get("x-mutli-building")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());

    let mut last_wait_log = std::time::Instant::now();

    loop {
        if last_wait_log.elapsed() >= Duration::from_secs(10) {
            tracing::info!("Still waiting for server to build download...");
            last_wait_log = std::time::Instant::now();
        }

        if let Some(wait_url) = &wait_url {
            let wait_full = if wait_url.starts_with("http") {
                wait_url.clone()
            } else {
                format!("https://www.karaoke-version.com{}", wait_url)
            };
            client
                .get(wait_full)
                .timeout(Duration::from_secs(30))
                .header("Cookie", cookie_header)
                .header("X-Requested-With", "XMLHttpRequest")
                .send()?;
        }

        let resp = client
            .get(&produced_url)
            .timeout(Duration::from_secs(30))
            .header("Cookie", cookie_header)
            .header("X-Requested-With", "XMLHttpRequest")
            .send()?;

        if let Some(file_href) = resp.headers().get("x-file-href") {
            return Ok(file_href.to_str()?.to_string());
        }

        if start.elapsed() > timeout {
            return Err(anyhow!("Timed out waiting for server to build download"));
        }
        sleep(Duration::from_millis(500));
    }
}

fn download_file(client: &Client, url: &str, path: &PathBuf) -> Result<()> {
    let mut resp = client.get(url).send()?;
    let mut file = std::fs::File::create(path)?;
    std::io::copy(&mut resp, &mut file)?;
    Ok(())
}

#[cfg(test)]
mod track_type_tests {
    use super::is_precount_track;

    #[test]
    fn click_is_a_regular_selectable_track() {
        assert!(!is_precount_track("Click"));
        assert!(!is_precount_track("Click Track"));
        assert!(is_precount_track("Intro Count"));
        assert!(is_precount_track("Precount"));
    }

    #[test]
    fn extracts_click_from_description_containing_precount_control() {
        let description = "<div class='custom__mixer-track-caption-input'> Intro count</div><span class='custom__mixer-track-caption-name'>&nbsp;&nbsp;Click</span>";
        assert_eq!(super::normalize_metadata_track_name(description), "Click");
    }
}

trait Checkable {
    fn is_checked(&self) -> bool;
}

impl<'a> Checkable for Element<'a> {
    fn is_checked(&self) -> bool {
        match self.attributes.as_ref() {
            Some(attrs) => attrs.contains(&String::from("checked")),
            None => false,
        }
    }
}
