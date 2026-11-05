# KVUI C22 — change summary

## C22: HTTP track-identity / stale-mix hardening

- **HTTP cookie handling:** introduced Reqwest cookie-jar behavior in the HTTP session instead of manually pinning Cookie headers.
- **HTTP diagnostics:** added readiness-stage and MP3 response logging, including HTTP status, response time, ETag, Last-Modified, Date, and content length.
- **Requested filename and key:** HTTP readiness polling now requires the server-provided MP3 link filename to match the intended instrument and transposition; mismatches continue polling instead of downloading immediately.
- **Server-resource freshness:** compares MP3 Last-Modified against the time the instrument was first requested, allowing a two-second timestamp tolerance. Missing/unparseable Last-Modified is treated as unverifiable and causes retry/failure.
- **ETag reuse:** rejects a resource whose ETag matches an earlier accepted instrument in the current song pass.
- **HTTP status:** MP3 response must be exactly HTTP 200 before it can be written.
- **Offline tests:** added focused tests for filename/key matching, time freshness, and ETag reuse.

## C22 handoff — command-line companion

- The original Rust CLI remains present in the C22 tracked source and is built as `kv_downloader.exe` (`-p cli`).
- A Windows CLI Release build and help-option smoke test passed. A selected-track visible Chromium test downloaded the single requested Click MP3 at +1 semitone.
- **Known existing display issue:** with `--tracks`, the successful CLI completion list still prints every song instrument rather than just those requested. This is documented, not fixed in source. The KVUI interactive application is the primary contribution.
- User-facing quick starts now document both applications; these documents do **not** alter the frozen C22 source/tag.

## Inherited from earlier verified work (not newly introduced in C22)

- Protected staging/publication rather than careless replacement of existing downloads.
- Key-specific output folders.
- Byte-for-byte duplicate detection and retry behavior.
- Browser-based download method, which independently waits for the expected instrument filename.

Review the **cumulative** original v0.5.0-to-C22 source changes with `git diff 96010cc..5d143bc`. Review the focused C20-to-C22 stale-resource change with `git diff 9416fad..5d143bc`. All 20 local commits are documented in `docs/COMPLETE-CHANGE-HISTORY.md`. The separately verified offline handoff archive contains formatted patch files; these are not committed here.
