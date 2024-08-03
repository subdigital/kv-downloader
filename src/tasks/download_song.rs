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

        tab.capture_screenshot(CaptureScreenshotFormatOption::Png, Some(1), None, false)?;

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
