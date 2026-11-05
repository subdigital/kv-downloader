# KVUI C22 — contribution overview for maintainers

This is an **unofficial derivative contribution** to Ben Scheirman's `subdigital/kv-downloader` v0.5.0 project, created by Daddidio. It does not imply approval or endorsement by the upstream maintainer. The original MIT license, attribution, and preceding Git history are retained.

## Scope

The original project already contained the CLI downloader, authentication and Chromium automation, and an experimental `kvui` terminal interface. The 20 commits after upstream v0.5.0 add or refine:

- KVUI track selection, input handling, download method choice, and artist/song/key output organization.
- Protected staging, retry/recovery logic, and existing-file handling.
- Filename/key identity and freshness safeguards for HTTP stem downloads, with conservative failure behavior.
- A selected-instrument CLI option; the original CLI remains available, but KVUI is the primary interface.

## Where to start

- [KVUI quick start](KVUI-C22-QUICK-START.md) — keyboard controls, HTTP/Chromium choice, output behavior, and recovery.
- [CLI quick start](CLI-C22-QUICK-START.md) — command examples and the known misleading subset-download completion list.
- [Problems and corrections](PROBLEMS-FOUND-AND-CORRECTIONS.md) — observed issues and changes.
- [Test results](TEST-RESULTS.md) and [known limitations](KNOWN-LIMITATIONS.md) — verification scope and remaining uncertainties.
- [20-commit chronology](COMPLETE-CHANGE-HISTORY.md), [upstream baseline](ORIGINAL-BASELINE.md), [HTTP identity bug report](BUG-REPORT-C22.md), and [C22 change summary](CHANGELOG.md).

## Review scope and provenance

The verified application source is **exactly commit** `5d143bc295c8f6e4a02a2c36fa4ee83c8feb4c5b` (contributor branch `kvui-c22-verified`). The documentation-only branch builds on this commit and changes **Markdown files only**. It is **not** a new C22 application release.

The upstream baseline is `96010cca46974f7ba902e3c0d43acaaabd32fcda` (v0.5.0). Compare the source with `git diff 96010cc..5d143bc` and the last HTTP safeguards with `git diff 9416fad..5d143bc`.

Both Windows applications passed Release builds and limited live download/listening checks on the contributor's development machine. Neither broad cross-platform compatibility nor mathematical verification of the actual instrument content has been established. The final added HTTP 200 guard was compiled and automatically tested but did not receive a separate live test with the archived GUI executable.

No purchased audio, raw diagnostic logs, credentials, browser profiles, or compiled executables are included in this GitHub documentation branch. A separate locally audited handoff archive is available if the maintainer requests it.
