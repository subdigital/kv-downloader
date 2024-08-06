use clap::Parser;
use dotenv::dotenv;
use kv_downloader::{driver, tasks};
use std::{env, error::Error, thread::sleep, time::Duration};

#[derive(Parser, Debug)]
#[command(version, about, long_about)]
struct Args {
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
}

fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    let args = Args::parse();
    tracing::debug!(args = format!("cli args: {:?}", args));

    let user = env::var("KV_USERNAME")
        .expect("Requires KV_USERNAME env variable. Did you add this to your .env file?");
    let pass = env::var("KV_PASSWORD")
        .expect("Requires KV_PASSWORD env variable. Did you add this to your .env file?");

    let config = driver::Config {
        domain: extract_domain_from_url(&args.song_url).expect("missing domain from url"),
        headless: args.headless,
        ..Default::default()
    };
    let driver = driver::Driver::new(config);
    driver.sign_in(&user, &pass)?;

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
