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
}

impl Driver {
    pub fn new(config: Config) -> Self {
        let browser = Browser::new(LaunchOptions {
            headless: config.headless,
            window_size: Some((1024, 768)),
            enable_logging: true,
            ..Default::default()
        })
        .expect("Unable to create headless chrome browser");

        Driver { config, browser }
    }

    pub fn type_fast(&self, tab: &Tab, text: &str) {
        for c in text.chars() {
            tab.send_character(&c.to_string())
                .expect("failed to send character");
        }
    }
}
