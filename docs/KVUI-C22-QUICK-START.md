# KVUI C22 — Quick Start (Windows)

**Primary application:** `kvui.exe` (interactive terminal interface). This source branch is a locally verified derivative of Ben Scheirman's `kv-downloader` v0.5.0, with contributions from Daddidio. “C22” names the local development checkpoint; Cargo's package version remains 0.5.0.

## Get started

1. Open PowerShell or Windows Terminal in the folder containing `kvui.exe` and run `./kvui.exe` (on Windows PowerShell, `./` and `.\` both work; `.\kvui.exe` is conventional).
2. Use **Login** if prompted, with your own Karaoke-Version account. Work only with tracks you have purchased or are authorized to access.
3. Select **Set Download Folder** to choose a location you can write to (defaults to your user's Downloads folder when not previously configured).
4. Select **Browse My Songs**, highlight a song with **Up/Down**, and press **Enter**.
5. In song details, all discovered tracks start **selected**. Use **Up/Down** to highlight a track and **Space** to include/exclude it. The selected-track count is shown above the list.
6. Adjust **Intro Click**, **Key**, and **Method** if necessary. The defaults are **Intro Click On**, **original key (0)**, and **HTTP**.
7. Press **Tab** to focus the Download panel and **Enter**. Confirm the method and select one of **KEEP EXISTING**, **REPLACE EXISTING**, or **CANCEL**. **KEEP EXISTING** is the safer default.
8. When finished, the completion screen offers **Open in File Explorer**, **Back to Songs**, or **Quit**. Audition a sample of each downloaded instrument: filenames and headers alone cannot prove audio identity.

## Keyboard controls

| Screen | Keys | Effect |
|---|---|---|
| Main menu | Up/Down, Enter | Highlight/activate an option |
| Main menu | `q` | Quit; **Esc does not exit from the main menu** |
| Song list | Up/Down, Enter | Select a purchased song |
| Song list | `/`, type, Enter or Esc | Filter songs by title/artist; leave filter editing |
| Song list | `s` | Cycle Recently added → Artist → Title sorting |
| Song list | Esc | Return to main menu |
| Song details | Up/Down, Space | Navigate tracks and toggle selection |
| Song details | `i` | Toggle intro count-in click |
| Song details | `b` | Toggle HTTP / visible Chromium downloader |
| Song details | `+` / `-` or Right/Left | Shift key one semitone, from **−4 to +4** |
| Song details | Tab | Switch focus between Tracks and Download |
| Song details | Enter (Download focused) | Open the download confirmation |
| Confirmation | Left/Right or Tab, Enter | Choose keep/replace/cancel; Esc cancels |
| Completed download | Up/Down, Enter | Open folder / return / quit |

## Main-menu options

| Option | What it does |
|---|---|
| Browse My Songs | Uses your saved catalog and can update it incrementally |
| Resync Songs List | Performs a fresh catalog synchronization |
| Reset Song Progress | Choose one song and clear its saved completion history for **all** count-in/key variants; doesn't mean deleting MP3s |
| Reset All Download Progress | Clears the entire saved progress file; **does not delete completed MP3s**, but later downloads can repeat work |
| Set Download Folder | Choose where future downloads are placed |
| Login / Logout | Account sign-in/out; **Logout also clears the local song catalog and saved download progress** |
| Incomplete Downloads (when present) | Inspect abandoned temporary staging folders, defer, or confirm guarded cleanup |
| Quit | Exit application |

## HTTP versus Chromium

- **HTTP (default):** uses authenticated HTTP requests and C22 filename/key, response-time, ETag, Last-Modified, and duplicate-content safeguards. Some legitimate files may be rejected if server headers are missing, misleading, or affected by clock skew.
- **Chromium:** uses automated visible browser interaction to obtain requested tracks. Do not interact with the browser while KVUI is driving it, except when the site explicitly requires verification or login. Use it as an alternate method when HTTP is unreliable.
- Both modes rely on the website's behavior. Neither can mathematically verify the musical instrument within an MP3; listen to the results.

## Output organization and overwrite safety

KVUI publishes into folders below the selected download root. Expected pattern:

```text
<DownloadRoot>\<Artist>\<Song>\...MP3 files...
<DownloadRoot>\<Artist>\<Song>-2\...MP3 files...
<DownloadRoot>\<Artist>\<Song>+2\...MP3 files...
```

The app sanitizes names for Windows and checks song-source identity to reduce cross-song collisions. **Original key** has no suffix; negative transposition has a minus suffix; positive transposition has a plus suffix. These folder conventions apply to **KVUI**, not necessarily to the separate CLI.

The song download is first prepared in a guarded temporary staging directory. **KEEP EXISTING** leaves existing matching files unchanged; **REPLACE EXISTING** replaces matching files after validation; **CANCEL** makes no download. Do not choose replace merely because a file has a suspicious name: first listen and inspect.

## Interrupted downloads and recovery

An **Incomplete Downloads** prompt may appear on startup or in the main menu. **View Details** lists temporary files and locations; **Remind Me Later** leaves them alone; **Clean Up** asks for an additional confirmation. Cleanup is designed to avoid active, legacy, or unsafe staging folders and completed published files. It is still a deletion of selected abandoned *temporary* files: inspect before confirming, especially if another download is running.

## Troubleshooting

- **Not logged in / song not listed:** log in and use **Resync Songs List**.
- **Unexpected track / wrong sound:** listen to the actual MP3; if wrong, keep your current files, try a fresh test destination or the alternate method, and preserve diagnostic details for the maintainer.
- **HTTP freshness failures / 503:** retry after a short interval or select Chromium. C22 may reject legitimate files if server metadata does not meet its conservative checks.
- **No tracks selected:** highlight at least one track and press Space before starting.
- **No output / folder problem:** verify **Set Download Folder** points to a writable directory.
- **Progress seems inconsistent:** progress resets clear records rather than files. Do not reset all progress as a routine troubleshooting step.
- **Chromium windows open even in HTTP mode:** In a reported November 5 C22 session, Chromium opened, logged in automatically, and closed while (1) opening/synchronizing the song list, (2) loading a selected song, and (3) **starting** a multi-stem download. **It did not reopen or log in separately for each stem.** The operation and multi-stem download succeeded without manual login. This intermittent automatic browser fallback is **not evidence that Chromium was selected** as the download method. Record which operation caused a window; do not clear progress or repeat paid downloads merely to diagnose it. Cause remains unknown; time of day and server load have not been established as factors.

## Build from source (optional)

From the repository root with Rust and Cargo installed:

```powershell
cargo test --workspace --locked
cargo build --release -p kvui --locked
```

**Known boundaries:** C22 has been verified in live download testing on the development machine, but no independent clean-machine or cross-platform validation is claimed. The final HTTP status guard was included in the passing final build/tests; the earlier full live HTTP listening tests preceded that last guard.

A source build produces `target\release\kvui.exe`. The separately audited handoff uses a `-c22-verified` suffix; the executable is not included in this Git branch.
