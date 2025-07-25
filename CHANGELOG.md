# Changelog

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
