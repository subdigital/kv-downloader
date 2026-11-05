# Original upstream baseline and attribution

## Original project

- Project: `kv-downloader`, including the experimental `kvui` terminal interface.
- Original repository: https://github.com/subdigital/kv-downloader
- Original developer/maintainer: Ben Scheirman (Git history; other upstream contributors are retained in the original history).
- Original code license: MIT; the original `LICENSE` is included unchanged in the C22 source snapshot.
- **Our actual starting point:** upstream version **0.5.0**, October 27, 2026, Git commit abbreviated `96010cc`.
- C22 endpoint: commit `5d143bc295c8f6e4a02a2c36fa4ee83c8feb4c5b`, annotated tag `kvui-c22-verified`, November 5, 2026.
- **Exactly 20 local commits** follow upstream v0.5.0 on the path to C22; the contributor approves using the Git author name **Daddidio** for this work.

## Attribution boundaries

The original v0.5.0 already provided the command-line downloader, the experimental KVUI, browser automation, authentication, song-library handling, and other capabilities described by the original project. These are not presented as new contributions here.

Our modifications followed v0.5.0. The work primarily hardened browser/UI interactions, protected user downloads, organized song/key output, and improved direct HTTP track reliability. See `PROBLEMS-FOUND-AND-CORRECTIONS.md` and the full chronological commit list.

## Comparing the code

- The frozen C22 source checkpoint is commit `5d143bc295c8f6e4a02a2c36fa4ee83c8feb4c5b` on the contributor's `kvui-c22-verified` branch.
- Review the full tracked-source changes since original v0.5.0 with `git diff 96010cc..5d143bc`.
- Inspect the final HTTP stale-mix changes with `git diff 9416fad..5d143bc`.
- List the local commits with `git log --oneline 96010cc..5d143bc`.
- The separately audited offline handoff archive contains source ZIP files, formatted patches, and SHA-256 manifests. Those archives are not committed to GitHub.

This is a developer contribution proposal, not an official upstream release or an assertion of maintainer approval.
