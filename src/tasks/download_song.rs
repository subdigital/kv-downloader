use headless_chrome::{protocol::cdp::Page::CaptureScreenshotFormatOption, Element};

use crate::driver::Driver;
use std::{error::Error, time::Duration};

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

impl Driver {
    pub fn download_song(&self, url: &str, options: DownloadOptions) -> Result<(), Box<dyn Error>> {
        let tab = self.browser.new_tab()?;
        tab.set_default_timeout(Duration::from_secs(30));

        tab.navigate_to(url)?.wait_until_navigated()?;

        if options.count_in {
            let el = tab.wait_for_element("input#precount")?;
            if !el.is_checked() {
                el.click()?;
            }
        }

        self.adjust_pitch(options.transpose, &tab)?;

        Ok(())
    }
    fn adjust_pitch(&self, desired_pitch: i8, tab: &Tab) -> Result<(), Box<dyn Error>> {
        // pitch is remembered per-son on your account, so this logic cannot be deterministic. Instead
        // we''l try to infer the direction we need to go based on what the pitch is currently set to.
        let pitch_label = tab
            .wait_for_element("span.pitch__value")
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
        tab.find_element("a#pitch-link")
            .expect("can't find pitch link")
            .click()?;

        tracing::info!("Reloading tracks after pitching...");
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
