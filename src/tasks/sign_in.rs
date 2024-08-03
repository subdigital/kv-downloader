use crate::driver::Driver;
use std::error::Error;

impl Driver {
    pub fn sign_in(&self, user: &str, pass: &str) -> Result<(), Box<dyn Error>> {
        let url = format!("https://{}/my/login.html", self.config.domain);
        let tab = self.browser.new_tab()?;
        let _viewport = tab.navigate_to(&url)?;

        // fill out form
        // tab.wait_for_element("#frm_login")?.type_into(user)?;
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

        Ok(())
    }
}
