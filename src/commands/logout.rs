use crate::keystore;
use anyhow::Result;

pub fn run() -> Result<()> {
    keystore::Keystore::logout()
}
