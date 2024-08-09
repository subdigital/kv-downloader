use anyhow::Result;
use headless_chrome::protocol::cdp::Network::{Cookie, CookieParam};
use keyring::Entry;
use serde::{Deserialize, Serialize};

pub struct Keystore {}

const KEYSTORE_SERVICE: &str = "kv-downloader";
const KV_CREDENTIALS_KEY: &str = "KV_CREDENTIALS";

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Credentials {
    pub user: String,
    pub password: String,
}

impl Keystore {
    pub fn login(user: &str, password: &str) -> Result<Credentials> {
        let creds = Credentials {
            user: user.to_string(),
            password: password.to_string(),
        };
        let encoded_data = serde_json::to_vec(&creds)?;
        Entry::new(KEYSTORE_SERVICE, KV_CREDENTIALS_KEY)?.set_secret(&encoded_data)?;
        Ok(creds)
    }

    pub fn logout() -> Result<()> {
        if let Ok(entry) = Entry::new(KEYSTORE_SERVICE, KV_CREDENTIALS_KEY) {
            let _ = entry.delete_credential().ok();
        }
        Ok(())
    }

    pub fn get_credentials() -> Result<Credentials> {
        let entry = Entry::new(KEYSTORE_SERVICE, KV_CREDENTIALS_KEY)?;
        let encoded_data = entry.get_secret()?;
        let creds: Credentials = serde_json::from_slice(&encoded_data)?;
        Ok(creds)
    }

    #[allow(dead_code)]
    pub fn get_auth_cookie(user: &str) -> Result<CookieParam> {
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

    #[allow(dead_code)]
    pub fn set_auth_cookie(user: &str, cookie: &Cookie) -> Result<()> {
        let value = serde_json::to_vec_pretty(&cookie).expect("Unable to serialize cookie");
        Entry::new(KEYSTORE_SERVICE, user)?.set_secret(&value)?;
        Ok(())
    }
}
