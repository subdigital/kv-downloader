# KV Downloader C22 — Command-Line Quick Start (Windows)

**Companion application:** `kv_downloader.exe` (the original command-line program, with subsequent changes in shared download code). **KVUI is the recommended main interface.** The CLI is retained for existing command-line workflows and for developers who want to test HTTP or Chromium directly.

The CLI prints its internal product version as **0.5.0**; “C22” identifies the locally tested source checkpoint, not a change to the upstream crate version.

## Start and authenticate

Open PowerShell in the folder containing the executable:

```powershell
.\kv_downloader.exe --help
.\kv_downloader.exe auth
```

The `auth` command is the original application's sign-in workflow; follow the prompts and use your own Karaoke-Version account. The `logout` command clears stored sign-in state. Do not share your credentials or session/profile files.

## Examples (replace the URL with your purchased song's page)

**HTTP (default, original key):**

```powershell
.\kv_downloader.exe download "https://www.karaoke-version.com/custombackingtrack/ARTIST/SONG.html" --download-path "D:\Music\TestDownloads"
```

**Visible Chromium (tested), download only Click at +1 semitone:**

```powershell
.\kv_downloader.exe download "https://www.karaoke-version.com/custombackingtrack/ARTIST/SONG.html" --browser --tracks "Click" --transpose 1 --download-path "D:\Music\TestDownloads"
```

**Select multiple instruments:**

```powershell
.\kv_downloader.exe download "https://www.karaoke-version.com/custombackingtrack/ARTIST/SONG.html" --tracks "Click,Drum Kit,Bass" --transpose -2 --count-in
```

Track names must match names exposed by the purchased song (for example, some recordings use `Drum Kit`, while others may use a different name). Omitting `--tracks` requests all available tracks. `--count-in` is opt-in in the CLI, whereas the KVUI screen initially displays Intro Click **On**.

## Options

| Option | Function |
|---|---|
| `download SONG_URL` | Download a purchased backing-track song |
| `--browser` | Use Chromium automation; without this switch HTTP is used |
| `--headless` / `-H` | Run Chromium without a visible window (relevant with `--browser`) |
| `--tracks "Click,Bass"` | Select only the specified comma-separated instruments |
| `--transpose N` / `-t N` | Shift key by −4 through +4 semitones; default 0 |
| `--count-in` / `-c` | Include intro count-in; absent by default |
| `--download-path PATH` / `-d PATH` | Choose output folder |
| `--browser-idle-timeout-secs N` | Browser idle timeout; default 300 seconds |
| `--force-restart` | **Caution:** clears shared saved download progress; avoid for ordinary downloads |
| `--debug` | Enable debug-level logging (may include account URLs or local paths) |
| `auth` / `logout` | Manage original command-line app authentication |

Run `download --help` to view the exact options supported by the compiled binary.

## Known CLI completion-message issue (documented, not fixed)

**Observed in the C22 Chromium live test:** the CLI successfully downloaded only the selected `Click` MP3 at +1 semitone. However, its final message read `Done! All tracks downloaded successfully:` and listed every instrument in the song. This happens because the completion summary reports the song's full discovered track list, not just the requested subset.

**Interpretation:** the message is misleading for filtered downloads. It does **not** establish that all displayed instruments were downloaded. Confirm actual output files and, when relevant, listen to them. The implementation has deliberately been kept unchanged for this handoff; KVUI is the primary supported experience.

## Verification and limitations

- CLI Release build (`cargo build --release -p cli --locked`) and `--version`, `--help`, `download --help` smoke checks **passed** on the Windows development machine.
- A real **visible Chromium** test using *America — A Horse with No Name*, **Click**, **+1**, produced the correct single MP3; the tester reported the download passed and also observed the summary bug above.
- **A separate live HTTP test through the CLI executable was not performed.** HTTP is exercised more extensively through KVUI and the shared Rust core, but this is not an independent CLI HTTP verification.
- The CLI does not provide KVUI's interactive catalog, selectable song menu, confirmation choices, or documented artist/song/key folder-publishing interface. Do not assume that CLI output matches KVUI's folder structure.
- A clean-machine installation and other operating systems have not been verified.
- Do not share purchased stems, credentials, browser profiles, or unsanitized diagnostic logs with the maintainer.

## Build from source

```powershell
cargo build --release -p cli --locked
```

This creates `target\release\kv_downloader.exe`. The separately audited offline handoff uses a `-c22-verified` suffix for its Windows executable; no binary is included in this Git branch.
