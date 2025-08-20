use crate::download_progress::DownloadProgress;
use headless_chrome::{Browser, LaunchOptions, Tab};
use std::error::Error;

pub struct Config {
    pub domain: String,
    pub headless: bool,
    pub download_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            domain: "www.karaoke-version.com".to_string(),
            headless: false,
            download_path: None,
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
            ..Default::default()
        })
        .expect("Unable to create headless chromium browser");

        if let Some(download_path) = &config.download_path {
            tracing::info!("Setting download path to: {}", download_path);
            Driver::set_download_path(&browser, download_path)
                .expect("failed to set download path");
        }

        Driver { 
            config, 
            browser,
            progress: DownloadProgress::new(),
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

    pub fn type_fast(&self, tab: &Tab, text: &str) {
        for c in text.chars() {
            tab.send_character(&c.to_string())
                .expect("failed to send character");
        }
    }
}
