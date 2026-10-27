# Changelog

## 0.5.0

- Reuse a persistent Chrome profile so Fastly's bot-challenge clearance survives between runs
- Launch Chrome without automation flags
- Detect Fastly's challenge page by its title only, so normal pages (which load Fastly's security script) are not mistaken for it
- HTTP downloads reuse the verified browser's cookies and User-Agent
- Refresh the browser session and retry when a track is challenged mid-song
- Pace requests with jitter and stop a song after repeated consecutive failures

## 0.4.1

- Adds a way to pass additional flags to the headless chrome binary. Separate these arguments from kv_downloader args with a single `--`.

## 0.4.0

- Add a metadata file to track progress of downloads, allow a second run to continue where you left off
- Add retry logic for individual tracks with increasing backoff logic
- Add `--force` flag to ignore the progress and start over

## 0.3.3

- Increased timeouts

## 0.3.2

- Fixed UK downloads by making title matching case insensitive

## 0.3.1

- Fixed hang while specifying a download path
- Headless mode now works when specying a download path

## 0.3.0

- Now called "kv_downloader"
- Rewritten in Rust (mostly for practice, but also for easier maintenance on my part)
- Credentials now stored in OS-specific keychain. Login with `kv_downloader auth`. Clear credentials
with `kv_downloader logout`.
- Session cookie is saved and used for use on subsequent runs.

---

## 0.2

- Fixed usage of karaoke-version.co.uk
- Added `-d` to change download path
- Added `-h` for headless mode
- Added `-p` for pitch changes

---

## 0.1

Initial release
