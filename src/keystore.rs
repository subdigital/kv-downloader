use anyhow::Result;
use headless_chrome::protocol::cdp::Network::{Cookie, CookieParam};
use keyring::Entry;

#[allow(dead_code)]
pub struct Keystore {}

const KEYSTORE_SERVICE: &str = "kv-downloader";
const KV_USERNAME_KEY: &str = "KV_USERNAME";
const KV_PASSWORD_KEY: &str = "KV_PASSWORD";

pub struct Credentials {
    pub user: String,
    pub password: String,
}

impl Keystore {
    pub fn login(user: &str, password: &str) -> Result<Credentials> {
        Entry::new(KEYSTORE_SERVICE, KV_USERNAME_KEY)?.set_password(user)?;
        Entry::new(KEYSTORE_SERVICE, KV_PASSWORD_KEY)?.set_password(password)?;
        Ok(Credentials {
            user: user.to_string(),
            password: password.to_string(),
        })
    }

    pub fn logout() -> Result<()> {
        let user_entry = Entry::new(KEYSTORE_SERVICE, KV_PASSWORD_KEY)?;
        if let Ok(user) = user_entry.get_password() {
            Entry::new(KEYSTORE_SERVICE, &user)?.delete_credential()?;
        }
        user_entry.delete_credential()?;
        Entry::new(KEYSTORE_SERVICE, KV_USERNAME_KEY)?.delete_credential()?;
        Entry::new(KEYSTORE_SERVICE, KV_PASSWORD_KEY)?.delete_credential()?;
        Ok(())
    }

    pub fn get_credentials() -> Result<Credentials> {
        Ok(Credentials {
            user: Entry::new(KEYSTORE_SERVICE, KV_USERNAME_KEY)?.get_password()?,
            password: Entry::new(KEYSTORE_SERVICE, KV_PASSWORD_KEY)?.get_password()?,
        })
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
