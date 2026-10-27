use crate::download_progress::DownloadProgress;
use headless_chrome::protocol::cdp::Network::CookieParam;
use headless_chrome::{types::Bounds, Browser, LaunchOptions, Tab};
use std::error::Error;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_BROWSER_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub struct Config {
    pub domain: String,
    pub headless: bool,
    pub download_path: Option<String>,
    pub idle_browser_timeout: Duration,
    /// Chrome profile to reuse between runs. Keeps the site's bot-challenge
    /// clearance and sign-in. `None` uses a throwaway profile.
    pub profile_dir: Option<PathBuf>,
}

/// Where the persistent Chrome profile lives.
pub fn default_profile_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("kv-downloader").join("chrome-profile"))
}

impl Default for Config {
    fn default() -> Self {
        Config {
            domain: "www.karaoke-version.com".to_string(),
            headless: false,
            download_path: None,
            idle_browser_timeout: DEFAULT_BROWSER_IDLE_TIMEOUT,
            profile_dir: None,
        }
    }
}

pub struct Driver {
    pub config: Config,
    pub browser: Browser,
    pub progress: DownloadProgress,
}

impl Driver {
    pub fn new(config: Config) -> Self {
        let browser = Browser::new(LaunchOptions {
            headless: config.headless,
            window_size: Some((1440, 1200)),
            enable_logging: true,
            user_data_dir: config.profile_dir.clone(),
            // headless_chrome defaults this to true, which makes Chrome label
            // every page "Not Secure" and weakens TLS checks for no benefit.
            ignore_certificate_errors: false,
            // Avoid Chromium's own macOS Keychain prompts for temporary profiles.
            args: vec![
                OsStr::new("--password-store=basic"),
                OsStr::new("--use-mock-keychain"),
                OsStr::new("--disable-blink-features=AutomationControlled"),
            ],
            // Don't present as an automated browser; the site's bot check
            // keeps re-challenging sessions that do.
            ignore_default_args: vec![OsStr::new("--enable-automation")],
            // Keep the DevTools websocket alive during long downloads.
            idle_browser_timeout: config.idle_browser_timeout,
            ..Default::default()
        })
        .unwrap_or_else(|err| {
            let mode = if config.headless { "headless" } else { "visible" };
            panic!("Unable to create {mode} Chromium browser: {err}");
        });

        if let Some(download_path) = &config.download_path {
            tracing::info!("Setting download path to: {}", download_path);
            Driver::set_download_path(&browser, download_path)
                .expect("failed to set download path");
        }

        let download_path = config.download_path.clone();

        Driver {
            config,
            browser,
            progress: DownloadProgress::new_with_path(download_path.as_deref()),
        }
    }

    fn set_download_path(browser: &Browser, download_path: &str) -> Result<(), Box<dyn Error>> {
        let tab = browser
            .new_tab()
            .expect("couldn't open a new tab to set download behavior");

        let download_behavior_method = headless_chrome::protocol::cdp::Browser::SetDownloadBehavior {
            browser_context_id: None,
            behavior: headless_chrome::protocol::cdp::Browser::SetDownloadBehaviorBehaviorOption::Allow,
            download_path: Some(download_path.to_string()),
            events_enabled: None
        };
        tracing::debug!("call_method (set download behavior)");
        tab.call_method(download_behavior_method)?;

        Ok(())
    }

    /// Asks Chrome to shut down cleanly so the profile (cookies, challenge
    /// clearance) is flushed to disk. Dropping the driver kills the process.
    pub fn close(&self, tab: &Tab) {
        let close = headless_chrome::protocol::cdp::Browser::Close(None);
        if let Err(err) = tab.call_method(close) {
            tracing::debug!("Failed to close browser cleanly: {}", err);
        }
        std::thread::sleep(Duration::from_secs(2));
    }

    pub fn minimize_tab(&self, tab: &Tab) {
        if self.config.headless {
            return;
        }

        if let Err(err) = tab.set_bounds(Bounds::Minimized) {
            tracing::debug!("Failed to minimize browser window: {}", err);
        }
    }

    pub fn set_session_cookie(&self, tab: &Tab, value: &str) -> anyhow::Result<()> {
        tab.set_cookies(vec![CookieParam {
            name: "karaoke-version".to_string(),
            value: value.to_string(),
            url: Some(format!("https://{}", self.config.domain)),
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
        Ok(())
    }

    pub fn type_fast(&self, tab: &Tab, text: &str) {
        for c in text.chars() {
            tab.send_character(&c.to_string())
                .expect("failed to send character");
        }
    }
}
