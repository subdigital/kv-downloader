use crate::{keystore, prompt};
use anyhow::Result;

pub fn run() -> Result<()> {
    println!(
        r#"
        This will store your username & password securely using your operating system's keychain store.
        These credentials will only be used to pass to the browser during the sign-in process and will
        otherwise not leave this device.

        "#
    );

    let user = prompt::prompt("Username: ", false)?;
    let pass = prompt::prompt("Password: ", true)?;

    _ = keystore::Keystore::login(&user, &pass);

    Ok(())
}
