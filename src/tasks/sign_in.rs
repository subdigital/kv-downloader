use crate::keystore::Keystore;
use std::{thread::sleep, time::Duration};

use crate::driver::Driver;
use anyhow::Result;

impl Driver {
    pub fn sign_in(&self, user: &str, pass: &str) -> Result<()> {
        let tab = self.browser.new_tab()?;

        // navigate to the homepage
        tab.navigate_to(&format!("https://{}", self.config.domain))?
            .wait_until_navigated()?;

        // this doesn't seem to work yet...
        if let Some(cookie) = Keystore::get_auth_cookie().ok() {
            tracing::debug!("Cookies before:");
            for c in tab.get_cookies()? {
                tracing::debug!(cookie = format!("{}: {}", c.name, c.value), "🍪");
            }

            tracing::info!("Using previous cookie value");
            tracing::debug!(cookie = serde_json::to_string(&cookie).unwrap());

            // only set it if it's an authenticated session with a user id?
            if cookie.value.contains("|u-i:") {
                tracing::debug!("Setting cookie");
                tab.set_cookies(vec![cookie])
                    .expect("unable to set cookies");
                tracing::debug!("Reloading tab...");
                tab.reload(true, None)?;

                sleep(Duration::from_secs(2));

                tracing::debug!("Cookies after:");
                for c in tab.get_cookies()? {
                    tracing::debug!(cookie = format!("{}: {}", c.name, c.value), "🍪");
                }
            }

            // continue to check for login link in case this cookie isn't valid anymore
        }

        tracing::info!(user = user, "Logging in user");

        let login_link = tab
            .find_element(".navigation a[href='/my/login.html']")
            .ok();

        // if we don't have a login link, we're already signed in (from a cookie)
        if login_link.is_none() {
            tracing::info!(user = user, "Already signed in");
            return Ok(());
        }

        // visit login page
        login_link.unwrap().click()?;

        // fill out form
        tab.wait_for_element("#frm_login")
            .expect("couldn't find username input")
            .focus()?;
        self.type_fast(&tab, user);

        tab.wait_for_element("#frm_password")
            .expect("couldn't find password input")
            .focus()?;
        self.type_fast(&tab, pass);

        // submit
        tab.find_element("#sbm")
            .expect("couldn't find submit button")
            .click()?;

        tab.wait_until_navigated()?;

        sleep(Duration::from_secs(2));

        // save cookie for next time
        let cookies = tab.get_cookies()?;
        if let Some(session_cookie) = cookies.iter().find(|c| c.name == "karaoke-version") {
            tracing::info!("Saving session cookie for next time");
            tracing::debug!(
                cookie = format!("{}: {}", session_cookie.name, session_cookie.value),
                "🍪"
            );
            Keystore::set_auth_cookie(session_cookie)?;
        }

        Ok(())
    }
}
