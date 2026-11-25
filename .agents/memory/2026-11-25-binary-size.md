# Binary size against the 7 MiB bar

**Date**: 2026-11-25
**Agent**: Claude Code
**Confidence**: High (measured on Linux x86_64 glibc; Windows estimated)
**Status**: Archived — the owner, 2026-11-25: "Để mode fat như cổng an toàn; hiện tại vẫn okay": `lto = "fat"`, the 7 MiB bar kept
**Source**: the owner: "bản build hiện đã gần 6.9MB, cũng khá chạm ngưỡng rồi?"
**Review-by**: 2026-12-31

## Problem

`scripts/smoke.sh` refuses a release binary over 7 MiB (7,340,032 bytes). beta.4 is close.

## Finding

- beta.3 release binaries (smoke log, run 45439606639): Windows 6,680,064 (largest),
  linux-musl 6,035,368, linux-gnu 5,923,664, macOS Intel 5,417,720, ARM 4.9–5.1 MB.
- beta.4 at `3c7600a`, linux-gnu: 6,196,432 (+4.6 % on beta.3). Windows at the same
  growth: ~6.99 MB = ~95 % of the bar.
- Where it goes (cargo bloat, .text): xdevcmd 1.4 MiB, std 1.2, xdevcmd-core 0.6, zip 0.25,
  toml 0.19, serde 0.13, zune_jpeg 0.10, zopfli 0.07, zlib_rs 0.07.
- Levers, measured (linux-gnu):

  | Profile | Size | vs now | Runtime |
  |---|---|---|---|
  | `lto = "thin"` (now) | 6,196,432 | — | — |
  | `lto = "fat"` | 5,892,584 | −4.9 % | same; build ~+20 s |
  | `lto = "fat"`, deps `opt-level = "s"` | 5,750,400 | −7.2 % | per-package levels are mostly lost under fat LTO |
  | `lto = "fat"`, `opt-level = "s"` | 4,416,000 | −29 % | list 100,000: 227 → 332 ms (+46 %); plan unchanged |

- `panic = "abort"` is out: a background thread's panic is meant to leave xdevcmd
  running (`app/run.rs`, `crash.rs`), which needs unwinding.

## Recommendation

1. `lto = "fat"` now: free size, no runtime cost.
2. Keep the 7 MiB bar; it is a tripwire. Raise it only with a stated reason (e.g. a
   feature that needs it), at a release cut, logged in the plan.
3. Not `opt-level = "s"`: listing is the hot path.
