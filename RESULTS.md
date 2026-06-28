# Results log

Leaderboard of recorded submissions. Full narratives live in
[`history/entries/`](history/entries/).

Every submission must be **≥128-bit secure** (classical core-SVP, checked by the built-in
lattice estimator). SCORE is median wall-clock nanoseconds for one programmable bootstrap
(lower is better), comparable only on the fixed reference runner.

**Current record: 49499417** (@10d9e, entry 0000)

| # | date | author | SCORE | Δ vs record | commit | entry | note |
|---|------|--------|-------|-------------|--------|-------|------|
| 0000 | 2026-06-28 | @10d9e | 49499417 | — (baseline) | `secure` | [0000](history/entries/0000-baseline.md) | 128-bit-secure baseline: f64-FFT CGGI PBS, n=1024 (LWE 131-bit) / N=2048 (GLWE 164-bit) |
