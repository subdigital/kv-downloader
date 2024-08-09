use anyhow::Result;
use std::io::{stdin, stdout, Write};

pub fn prompt(msg: &str, secure: bool) -> Result<String> {
    print!("{}", msg);
    stdout().flush()?;

    if secure {
        let pass = rpassword::read_password()?;
        Ok(pass)
    } else {
        let mut buf = String::new();
        stdin().read_line(&mut buf)?;
        Ok(buf.trim().to_string())
    }
}
