# Problems found, corrective changes, and evidence

**Scope:** work after upstream `0.5.0` (`96010cc`), through locally verified C22. Rows distinguish observed failures, protective improvements, and optional usability enhancements. Not every item below indicates a defect confirmed in the upstream program; some are improvements driven by our particular workflow. **Some problems emerged while exercising local additions, especially HTTP-first downloading. This is not an allegation that all issues existed in upstream v0.5.0.**

## Browser-downloader identity and completion (Nov 3, 2026)

**Observed risk/problem:** the browser track list included a `Click` entry that needed reliable extraction; a page could still offer a previous instrument's MP3 link while preparing a new selection; the UI could mark progress before a new, complete file was established. Retry clicks could disturb the solo selection.

**Changes:** corrected browser extraction of `Click`; validated offered filenames against the requested instrument; verified nonempty, newly downloaded files; checked exactly one solo button is active; preserved the first-request timestamp across retries; detected completed files before retrying; added clearer download errors and completion-aware waiting. The CLI also gained optional selected-track support. Visible-Chromium behavior was explicitly tested as an experimental mode.

**Evidence:** chronological commits `2770ce5` through `7232212`; a complete 11-track browser run of *America — A Horse with No Name* had filenames and audio verified by listening. Browser mode continues to be a fallback.

## Interactive terminal UI input (Nov 3–4, 2026)

**Observed problem:** Escape could exit the main menu unexpectedly; terminal event handling needed to avoid non-key-press events being treated as input.

**Changes:** changed Escape behavior (Correction 12), restricted processing to key-press events (Correction 13).

**Evidence:** commits `14f2a64` and `9d83dfe`. These UI items were not re-tested on a clean second machine.

## Protecting downloaded files and recovery (Nov 4, 2026)

**Identified risk/need:** incomplete or incorrect batches must not casually overwrite completed purchased stems. Stale or abandoned staging folders need controlled recovery or cleanup.

**Changes:** protected staging and existing-file publication (Correction 14); inspect/recover/cleanup for incomplete downloads with guards; selected-track count in UI.

**Evidence:** commits `d2e0a70`, `122ed23`, and `47e377c`. Offline regression tests passed; live C22 runs reported protected publication and no unintended replacement. The developer should inspect the source and tests for boundary conditions.

## Download workflow and file organization (Nov 4, 2026)

**Need/choice:** direct HTTP selected as the default KVUI download path, while retaining browser functionality. Improve filing of songs and key variants.

**Changes:** HTTP default selection, artist/song folder organization, verification of exact requested transposition in filenames, and dedicated key-specific song folders.

**Evidence:** commits `7fedca1`, `6080196`, `c14df3a`, `9416fad`. *Hotel California* three-track test at −2 semitones produced the correct instruments and key-specific output folder.

## Wrong audio beneath correct HTTP filenames (C22, Nov 5, 2026)

**Observed failure:** when requesting adjacent stems such as Click, Drum Kit, and Percussion, HTTP responses sometimes contained audio belonging to the preceding instrument—even though the new response had a plausible filename. Root cause was not isolated between server, client/session state, and request sequencing. Stale output could persist across KVUI sessions. Exact-byte duplicate checks caught some repetitions but could not certify musical identity or detect re-encodes.

**Changes:** cookie-jar session handling; diagnostic response/readiness logging; HTTP filename and requested-key validation; conservative Last-Modified freshness validation against the *first* instrument request; rejection of reused ETags from accepted earlier instruments; preserved bytewise duplicate checks; HTTP 200 requirement before file writing; bounded retries.

**Evidence:** final C22 tests: *A Horse with No Name* original key — 3/3 then 11/11 correct by listening; *Hotel California* at −2 — 3/3 correct by listening. The respective three live tests logged 2 + 6 + 3 = **11 timestamp-freshness rejections**, with correct eventual audio. Automated Rust regression tests and Release build passed. The live C22C-2 listening tests preceded the final explicit HTTP 200 response guard; the final guarded build was not separately live-tested through the GUI.

**Limitation:** no audio-signal classifier exists; a newly generated wrong mix with fresh headers could pass. Valid cached files and clock-skew scenarios can generate false rejections. These are safeguards, not mathematical proof of audio identity.

## What remains to be reviewed by the maintainer

- Whether each enhancement is general-purpose enough for upstream use.
- Whether the server's Last-Modified and ETag semantics justify keeping or adjusting the freshness policy.
- Additional clean-machine and cross-platform testing, and positive-key/other-song live tests.
- Integration with existing upstream design conventions and contribution process.

Inspect the complete tracked-source differences with `git diff 96010cc..5d143bc`. A formatted patch is also included in the separately audited offline handoff archive.
