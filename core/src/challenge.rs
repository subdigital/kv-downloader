use std::error::Error;
use std::fmt::Display;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CHALLENGE_TITLE: &str = "<title>client challenge</title>";

/// True if a response body is the Fastly "Client Challenge" interstitial.
///
/// Plain HTTP clients receive it as a regular 200 HTML page, so the body is
/// the only reliable signal. Don't match on `/_fs-ch-` asset paths: normal
/// pages load Fastly's `security.js` from there too.
pub fn is_challenge_html(html: &str) -> bool {
    html.to_lowercase().contains(CHALLENGE_TITLE)
}

/// Returned when the site answers an HTTP request with the bot challenge.
#[derive(Debug)]
pub struct ChallengeError;

impl Display for ChallengeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("The site is asking for browser verification")
    }
}

impl Error for ChallengeError {}

/// Adds up to 50% random extra delay so requests don't arrive on a fixed beat.
pub fn jitter(base: Duration) -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    base + base.mul_f64(f64::from(nanos % 1000) / 2000.0)
}

/// Rings the terminal bell and, on macOS, posts a notification.
pub fn notify_user(message: &str) {
    eprint!("\x07");
    if cfg!(target_os = "macos") {
        let script = format!(
            "display notification \"{}\" with title \"kv-downloader\"",
            message.replace('\\', "").replace('"', "'")
        );
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_fastly_challenge_page() {
        assert!(is_challenge_html(
            "<html><head><title>Client Challenge</title></head></html>"
        ));
    }

    #[test]
    fn ignores_normal_pages_that_load_fastly_security_script() {
        assert!(!is_challenge_html(
            "<title>Neon Moon</title><script src=\"/_fs-ch-1T1wmsGaOgGaSxcX/assets/security.js\"></script>"
        ));
    }

    #[test]
    fn ignores_pages_that_only_mention_fastly() {
        assert!(!is_challenge_html(
            "<html><body><img src=\"https://cdn.fastly.net/a.png\"></body></html>"
        ));
    }

    #[test]
    fn jitter_stays_within_half_again() {
        let base = Duration::from_secs(4);
        let delay = jitter(base);
        assert!(delay >= base && delay <= base + base / 2);
    }
}
