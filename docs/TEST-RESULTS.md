# KVUI C22 — verification summary

## Automated verification

- `cargo test --workspace --locked` — PASSED (reported from development machine).
- `cargo build --release -p kvui --locked` — PASSED.
- Git diff validation — PASSED before commit/tag.
- Archived C22 GUI executable verified by SHA-256.
- `cargo build --release -p cli --locked` — PASSED (reported from development machine).
- CLI `--version`, `--help`, and `download --help` smoke checks — PASSED.

**Final-change boundary:** The C22C-2 live HTTP listening tests were performed *before* the explicit HTTP 200 guard was added. The final workspace tests and Release build passed with that guard, but the archived C22 GUI executable was **not separately live-downloaded after that last change**. No dedicated HTTP-status regression test was reported.

## Live downloading and listening verification

| Test | Method | Transposition | Outcome |
|---|---|---|---|
| America — A Horse with No Name, Click / Drum Kit / Percussion | HTTP C22C-2 | 0 | 3 of 3 filenames and actual instrument audio correct; 2 server-time stale-resource rejections. |
| America — A Horse with No Name, complete 11-track set | HTTP C22C-2 | 0 | 11 of 11 filenames and actual instrument audio correct; 6 server-time stale-resource rejections. |
| Eagles — Hotel California, 3 acoustic-guitar-related stems | HTTP C22C-2 | -2 | 3 of 3 instrument and transposition results correct; 3 server-time stale-resource rejections; files in correct -2 key-specific folder. |
| America — A Horse with No Name, complete 11-track set | Browser method, before C22C-2 | 0 | 11 of 11 verified correct by listening. |

## CLI companion live test (after C22 tag)

- **Method:** Command-line executable, `--browser` (visible Chromium), selected `--tracks Click`, `--transpose 1`.
- **Song:** America — A Horse with No Name (purchased copy).
- **Result:** CLI user reported PASSED, with exactly **one requested Click MP3** created and verified, including listening; the final completion message incorrectly listed all instruments.
- **Limit:** The separate CLI executable has not been independently live-tested in HTTP mode; KVUI's HTTP testing uses the shared core.

**Scope:** 17 correct HTTP track outcomes across the three final C22C-2 test runs (some stems deliberately downloaded in more than one run); 11 freshness rejections across those runs. A successful test is not proof of perfect behavior for all songs or server states.

## Method

- Filenames, transposition suffixes, output folder placement, retry behavior, and success counts were checked.
- Actual instruments were checked by listening to the MP3 content, not by automated content classification.
- Diagnostic logs were inspected for ETag, Last-Modified, HTTP 503 failures, and retries.
- Raw logs are retained privately by the tester and excluded from this public GitHub branch to avoid disclosing local paths, account details, and service URLs.

## Not yet verified

- A post-final-guard live HTTP download of the archived GUI executable and a dedicated HTTP-status test.
- Clean-machine build/install or Windows signing.
- Independent ETag-only rejection in a live run (the timestamp check generally rejected first).
- Reliable operation on all purchased songs, all transposition values, or future site versions.
- Automated, semantic validation that audio corresponds to the instrument named in the MP3.
