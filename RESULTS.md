# Results log

Leaderboard of recorded submissions. Full narratives live in
[`history/entries/`](history/entries/).

Every submission must be **≥128-bit secure** (standard lattice-estimator model — primal-uSVP
with BDGL16 sieving gate-count — checked by the built-in estimator). SCORE is median
wall-clock nanoseconds for one programmable bootstrap (lower is better), comparable only on
the fixed reference runner.

**Current record: 20006084** (@10d9e, entry 0001)

| # | date | author | SCORE | Δ vs record | commit | entry | note |
|---|------|--------|-------|-------------|--------|-------|------|
| 0000 | 2026-06-28 | @10d9e | 49499417 | — (baseline) | `secure` | [0000](history/entries/0000-baseline.md) | 128-bit f64-FFT CGGI baseline at N=2048 (very comfortable noise margin) |
| 0001 | 2026-06-28 | @10d9e | 32413625 | -17085792 (new record) | `rstfhe` | [0001](history/entries/0001--10d9e.md) | rs-tfhe-style N=1024 params (n=768) — ~35% faster, 132/129.6-bit, σ-margin ~4.7 bits |
| 0002 | 2026-06-29 | @10d9e | 20006084 | -29493333 (new record) | `fastpbs` | [0002](history/entries/0002--10d9e.md) | hand radix-4 SIMD FFT + split-format spectra + branchless decompose + SIMD untwist/MAC (rs-tfhe-informed) |
