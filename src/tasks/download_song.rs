use headless_chrome::{Element, Tab};

use crate::driver::Driver;
use std::fmt::Display;
use std::{error::Error, thread::sleep, time::Duration};

pub struct DownloadOptions {
    pub count_in: bool,
    pub transpose: i8,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        DownloadOptions {
            count_in: false,
            transpose: 0,
        }
    }
}

#[derive(Debug)]
pub enum DownloadError {
    NotPurchased,
    NotASongPage,
}
impl Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPurchased => f.write_str("This track has not been purchased"),
            Self::NotASongPage => f.write_str("This doesn't look like a song page. Check the url."),
        }
    }
}
impl Error for DownloadError {}

impl Driver {
    pub fn download_song(&self, url: &str, options: DownloadOptions) -> Result<(), Box<dyn Error>> {
        let tab = self.browser.new_tab()?;
        tab.set_default_timeout(Duration::from_secs(30));

        tab.navigate_to(url)?.wait_until_navigated()?;

        if !self.is_a_song_page(&tab) {
            return Err(Box::new(DownloadError::NotASongPage) as Box<dyn Error>);
        }

        if !self.is_downloadable(&tab) {
            return Err(Box::new(DownloadError::NotPurchased) as Box<dyn Error>);
        }

        if options.count_in {
            let el = tab.wait_for_element("input#precount")?;
            if !el.is_checked() {
                el.click()?;
            }
        }

        self.adjust_pitch(options.transpose, &tab)?;

        self.solo_and_download_tracks(&tab)?;

        Ok(())
    }

    fn solo_and_download_tracks(&self, tab: &Tab) -> Result<(), Box<dyn Error>> {
        let solo_button_sel = ".track__controls.track__solo";
        let solo_buttons = tab.find_elements(solo_button_sel)?;
        let download_button = tab.find_element("a.download")?;
        let track_names = Driver::extract_track_names(&tab)?;

        tab.enable_debugger()?;
        for (index, solo_btn) in solo_buttons.iter().enumerate() {
            let track_name = track_names[index].clone();
            tracing::info!("Processing track {}", track_name);
            solo_btn.click()?;
            sleep(Duration::from_secs(2));

            download_button.click()?;
            tracing::info!("Waiting for download...");
            sleep(Duration::from_secs(2));
            tab.wait_for_element(".begin-download")
                .expect("Timed out waiting for download.");

            tab.find_element("button.js-modal-close")?.click()?;
            sleep(Duration::from_secs(4));
        }

        Ok(())
    }

    pub fn extract_track_names(tab: &Tab) -> Result<Vec<String>, Box<dyn Error>> {
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
                .map(|v| v.to_string().replace("\\n", " ").replace("\"", ""))
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

    fn is_downloadable(&self, tab: &Tab) -> bool {
        // if the download button also has the addtocart class, then this hasn't been purchased
        let el = tab.find_element("a.download.addtocart").ok();
        match el {
            Some(_) => false,
            None => true,
        }
    }

    fn adjust_pitch(&self, desired_pitch: i8, tab: &Tab) -> Result<(), Box<dyn Error>> {
        // pitch is remembered per-son on your account, so this logic cannot be deterministic. Instead
        // we''l try to infer the direction we need to go based on what the pitch is currently set to.
        let pitch_label = tab
            .find_element("span.pitch__value")
            .expect("can't find pitch value");
        let pitch_up_btn = tab
            .find_element("div.pitch button.btn--pitch[title='Key Up']")
            .expect("can't find pitch up button");
        let pitch_down_btn = tab
            .find_element("div.pitch button.btn--pitch[title='Key Down']")
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
