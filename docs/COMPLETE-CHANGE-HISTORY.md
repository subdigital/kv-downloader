# Complete local change history after original v0.5.0

**Baseline:** original upstream tag `0.5.0`, commit `96010cc` (October 27, 2026).
**Endpoint:** locally verified `kvui-c22-verified`, commit `5d143bc295c8f6e4a02a2c36fa4ee83c8feb4c5b` (November 5, 2026).
**Count:** 20 local commits, chronological. The `Daddidio` author identity reflects local Git commit metadata; upstream authorship is separate.

| # | Date | Commit | Subject |
|---:|---|---|---|
| 1 | Nov 3 | `2770ce5` | Fix browser track name extraction for Click |
| 2 | Nov 3 | `295bd94` | Validate browser download filenames against requested tracks |
| 3 | Nov 3 | `c24a472` | Verify browser downloads are new and nonempty |
| 4 | Nov 3 | `767d7cd` | Add optional track selection to command-line downloader |
| 5 | Nov 3 | `85c0f5a` | Experiment: keep Chromium visible during downloads |
| 6 | Nov 3 | `1af02ce` | Verify exclusive solo state and avoid toggling on retries |
| 7 | Nov 3 | `111124f` | Add context to browser download errors |
| 8 | Nov 3 | `fbb5015` | Preserve track freshness timestamp across download retries |
| 9 | Nov 3 | `de58a5c` | Detect completed track files before download retries |
| 10 | Nov 3 | `7232212` | Correction 11: completion-aware browser download waiting |
| 11 | Nov 3 | `14f2a64` | Correction 12: prevent Esc from exiting KVUI main menu |
| 12 | Nov 4 | `9d83dfe` | Correction 13: process only key-press events in KVUI |
| 13 | Nov 4 | `d2e0a70` | Correction 14: Safe download staging and existing-file protection |
| 14 | Nov 4 | `122ed23` | Recover and safely clean incomplete downloads |
| 15 | Nov 4 | `47e377c` | Display selected track count |
| 16 | Nov 4 | `7fedca1` | Use HTTP as default download method |
| 17 | Nov 4 | `6080196` | Organize downloads into artist and song folders |
| 18 | Nov 4 | `c14df3a` | Fix transposed filename and exact-key verification (C19) |
| 19 | Nov 4 | `9416fad` | Add key-specific song download folders (C20) |
| 20 | Nov 5 | `5d143bc` | C22: verify HTTP stem identity and freshness |

These subjects are taken from the local Git log, not reconstructed from informal chat notes. The 20-commit history can be reproduced with `git log --oneline 96010cc..5d143bc`. The original-to-C22 patch is the **final net source difference** across the 20 commits; it is **not** a 20-patch series and does not preserve intermediate experiments or complete Git topology. The chronological list above preserves commit subjects. The maintainer can request the reviewed branch or a verified `git format-patch` series if individual patches/authorship metadata are needed.

## Distinguishing original functionality from local changes

Do not attribute existing upstream v0.5.0 features (e.g., authentication, catalog browsing, experimental KVUI, or the original browser-based downloader) to these 20 commits. Refer to the original upstream README and license for existing features.
