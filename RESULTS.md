# Results log

Leaderboard of recorded submissions. Full narratives live in
[`history/entries/`](history/entries/).

SCORE is median wall-clock nanoseconds for one programmable bootstrap (lower is better),
comparable only on the fixed reference runner.

**Current record: 12023834** (@10d9e, entry 0001)

| # | date | author | SCORE | Δ vs record | commit | entry | note |
|---|------|--------|-------|-------------|--------|-------|------|
| 0000 | 2026-06-28 | @10d9e | 14186041 | — (baseline) | `f59fe38` | [0000](history/entries/0000-baseline.md) | Original baseline before tuning: f64-FFT CGGI PBS at blind-rotation length n=600 (~14 ms) |
| 0001 | 2026-06-28 | @10d9e | 12023834 | -2162207 (new record) | `f59fe38` | [0001](history/entries/0001-baseline.md) | Tune blind-rotation length n=600→500 (dominant cost is the n CMux external products) |
