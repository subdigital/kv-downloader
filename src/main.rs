use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand};
use dotenv::dotenv;
use keystore::Credentials;
use std::env;
use std::{thread::sleep, time::Duration};

use kv_downloader::{driver, tasks};

mod keystore;
mod prompt;

#[derive(Debug, Parser)]
#[command(name = "kv-downloader")]
#[command(version, about, long_about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Auth,
    Logout,
    #[command(arg_required_else_help = true)]
    Download(DownloadArgs),
}

#[derive(Debug, Args)]
#[command(flatten_help = true)]
struct DownloadArgs {
    song_url: String,

    #[arg(
        short = 'H',
        long,
        help = "Set this flag to launch the browser headless."
    )]
    headless: bool,

    #[arg(short, long)]
    download_path: Option<String>,

    #[arg(
        short,
        long,
        help = "Transpose the key of all tracks (i.e. -1 or 1)",
        value_parser = clap::value_parser!(i8).range(-4..=4),
        default_value = "0"
    )]
    transpose: Option<i8>,

    #[arg(short, long, help = "Whether to count in an intro for all tracks")]
    count_in: bool,

    #[arg(long, help = "Enable debug logs")]
    debug: bool,
}

fn main() -> Result<()> {
    dotenv().ok();
    let cli = Cli::parse();
    match cli.command {
        Commands::Auth => start_auth()?,
        Commands::Logout => keystore::Keystore::logout()?,
        Commands::Download(args) => start_download(args)?,
    }

    Ok(())
}

fn start_download(args: DownloadArgs) -> Result<()> {
    let credentials = credentials_from_env()
        .ok_or(keystore::Keystore::get_credentials())
        .map_err(|_| anyhow!("Must call `kv-downloader auth` first"))?;

    tracing_subscriber::fmt()
        .with_max_level(if args.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .init();
    tracing::debug!(args = format!("cli args: {:?}", args));

    let config = driver::Config {
        domain: extract_domain_from_url(&args.song_url).expect("missing domain from url"),
        headless: args.headless,
        download_path: args.download_path,
    };
    let driver = driver::Driver::new(config);
    driver.sign_in(&credentials.user, &credentials.password)?;

    let download_options = tasks::download_song::DownloadOptions {
        count_in: args.count_in,
        transpose: args.transpose.unwrap_or(0),
    };
    driver.download_song(&args.song_url, download_options)?;

    sleep(Duration::from_secs(10));

    Ok(())
}

fn extract_domain_from_url(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(|h| h.to_string()))
}

fn start_auth() -> Result<()> {
    println!(
        r#"
        This will store your username & password securely using your operating system's keychain store.
        These credentials will only be used to pass to the browser during the sign-in process and will
        otherwise not leave this device.

        "#
    );

    let user = prompt::prompt("Username: ", false)?;
    let pass = prompt::prompt("Password: ", true)?;

    _ = keystore::Keystore::login(&user, &pass);

    Ok(())
}

fn credentials_from_env() -> Option<Credentials> {
    env::var("KV_USERNAME")
        .and_then(|user| match env::var("KV_PASSWORD") {
            Ok(password) => Ok(Credentials { user, password }),
            Err(e) => Err(e),
        })
        .ok()
}
