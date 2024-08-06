mod server;

use headless_chrome::Browser;
use std::error::Error;

use server::Server;

use kv_downloader::{
    driver::{Config, Driver},
    tasks::download_song::DownloadOptions,
};

#[test]
fn extracts_track_names() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let driver = Driver::new(Config {
        headless: true,
        ..Default::default()
    });

    let tab = driver.browser.new_tab().unwrap();
    let file_server = Server::with_dumb_html(include_str!("./fixtures/cherub-rock.html"));
    tab.navigate_to(&file_server.url())?;
    tab.wait_until_navigated()?;

    let names = Driver::extract_track_names(&tab)?;

    assert_eq!(
        names,
        vec![
            "Click".to_string(),
            "Drum Kit".to_string(),
            "Bass".to_string(),
            "Electric Guitar (intro)".to_string(),
            "Rhythm Electric Guitar".to_string(),
            "Lead Electric Guitar 1".to_string(),
            "Lead Electric Guitar 2".to_string(),
            "Backing Vocals".to_string(),
            "Lead Vocal".to_string(),
        ]
    );

    Ok(())
}
