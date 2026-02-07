use std::{env, thread::sleep, time::Duration};

use crate::{
    driver,
    keystore::{self, Credentials},
    tasks,
};
use anyhow::{anyhow, Result};
use clap::{arg, command, Args};

#[derive(Debug, Args)]
#[command(flatten_help = true)]
pub struct DownloadArgs {
    pub song_url: String,

    #[arg(
        short = 'H',
        long,
        help = "Set this flag to launch the browser headless."
    )]
    pub headless: bool,

    #[arg(short, long)]
    pub download_path: Option<String>,

    #[arg(
        short,
        long,
        help = "Transpose the key of all tracks (i.e. -1 or 1)",
        value_parser = clap::value_parser!(i8).range(-4..=4),
        default_value = "0",
        allow_hyphen_values = true,
    )]
    pub transpose: Option<i8>,

    #[arg(short, long, help = "Whether to count in an intro for all tracks")]
    pub count_in: bool,

    #[arg(long, help = "Force restart, ignoring any previous download progress")]
    pub force_restart: bool,

    #[arg(
        long,
        help = "How long Chrome can be idle without events before the connection is closed (seconds)",
        default_value = "300"
    )]
    pub browser_idle_timeout_secs: u64,
}

pub struct Download {}

impl Download {
    pub fn run(args: DownloadArgs) -> Result<()> {
        Download::start_download(args)
    }

    fn start_download(args: DownloadArgs) -> Result<()> {
        let credentials = credentials_from_env().unwrap_or(
            keystore::Keystore::get_credentials().map_err(|e| {
                tracing::error!("credential error: {}", e);
                anyhow!("Must call `kv-downloader auth` first")
            })?,
        );

        tracing::debug!(args = format!("cli args: {:?}", args));

        let config = driver::Config {
            domain: extract_domain_from_url(&args.song_url).expect("missing domain from url"),
            headless: args.headless,
            download_path: args.download_path,
            idle_browser_timeout: Duration::from_secs(args.browser_idle_timeout_secs),
        };
        let driver = driver::Driver::new(config);

        // Handle resume/restart logic
        if args.force_restart {
            tracing::info!("Force restart requested, clearing previous progress");
            driver.progress.clear()?;
        } else if driver.progress.is_same_url(&args.song_url)? {
            let completed = driver.progress.get_completed_tracks()?;
            if !completed.is_empty() {
                tracing::info!(
                    "Resuming previous download. Already completed {} tracks:",
                    completed.len()
                );
                for track in &completed {
                    tracing::info!("  ✓ {}", track);
                }
            }
        } else if !driver.progress.get_completed_tracks()?.is_empty() {
            // Different URL, clear the old progress
            tracing::info!("Different song detected, clearing previous progress");
            driver.progress.clear()?;
        }

        driver.sign_in(&credentials.user, &credentials.password)?;

        let download_options = tasks::download_song::DownloadOptions {
            count_in: args.count_in,
            transpose: args.transpose.unwrap_or(0),
            selected_tracks: None,
        };
        driver.download_song(&args.song_url, download_options)?;

        sleep(Duration::from_secs(10));

        Ok(())
    }
}

fn credentials_from_env() -> Option<Credentials> {
    env::var("KV_USERNAME")
        .and_then(|user| match env::var("KV_PASSWORD") {
            Ok(password) => Ok(Credentials { user, password }),
            Err(e) => Err(e),
        })
        .ok()
}

fn extract_domain_from_url(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|h| h.to_string()))
}
