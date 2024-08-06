use headless_chrome::protocol::cdp::Network::{Cookie, CookieParam};
use keyring::Entry;
use std::error::Error;

#[allow(dead_code)]
pub struct Keystore {}

#[allow(dead_code)]
const KEYSTORE_SERVICE: &str = "kv-downloader";

#[allow(dead_code)]
impl Keystore {
    pub fn get_auth_cookie(user: &str) -> Result<CookieParam, Box<dyn Error>> {
        let secret = Entry::new(KEYSTORE_SERVICE, user)?.get_secret()?;
        let cookie: Cookie = serde_json::from_slice(&secret).expect("Unable to deserialize cookie");

        // return a cookie param so it can be set on the tab type (get/set use differnet types)
        let cookie_param = CookieParam {
            name: cookie.name,
            value: cookie.value,
            url: None,
            domain: Some(cookie.domain),
            secure: Some(cookie.secure),
            http_only: Some(cookie.http_only),
            same_site: cookie.same_site,
            path: Some(cookie.path),
            expires: Some(cookie.expires),
            priority: Some(cookie.priority),
            same_party: Some(cookie.same_party),
            source_scheme: Some(cookie.source_scheme),
            source_port: Some(cookie.source_port),
            partition_key: cookie.partition_key,
        };

        Ok(cookie_param)
    }

    pub fn set_auth_cookie(user: &str, cookie: &Cookie) -> Result<(), Box<dyn Error>> {
        let value = serde_json::to_vec_pretty(&cookie).expect("Unable to serialize cookie");
        Entry::new(KEYSTORE_SERVICE, user)?
            .set_secret(&value)
            .map_err(|e| Box::new(e) as Box<dyn Error>)
    }
}
