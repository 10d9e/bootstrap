# Results log

Leaderboard of recorded submissions. Full narratives live in
[`history/entries/`](history/entries/).

Every submission must be **≥128-bit secure** (standard lattice-estimator model — primal-uSVP
with BDGL16 sieving gate-count — checked by the built-in estimator). SCORE is median
wall-clock nanoseconds for one programmable bootstrap (lower is better), comparable only on
the fixed reference runner.

> **Challenge spec changed at entry 0003** to a **boolean gate bootstrap**
> (`REQUIRED_MESSAGE_BITS = 2`, the rs-tfhe / CGGI setting). Entries 0000–0002 used the earlier
> **4-message** spec (`message_bits = 3`, a tighter noise budget) and their scores are **not
> directly comparable** to 0003+.

**Current record: 14878208** (@10d9e, entry 0003 — boolean; beats rs-tfhe `gate_nand` 14.6–14.8 ms head-to-head)

| # | date | author | SCORE | spec | commit | entry | note |
|---|------|--------|-------|------|--------|-------|------|
| 0000 | 2026-06-28 | @10d9e | 49499417 | 4-msg | `secure` | [0000](history/entries/0000-baseline.md) | 128-bit f64-FFT CGGI baseline at N=2048 |
| 0001 | 2026-06-28 | @10d9e | 32413625 | 4-msg | `rstfhe` | [0001](history/entries/0001--10d9e.md) | rs-tfhe-style N=1024 params (n=768), 132/129.6-bit |
| 0002 | 2026-06-29 | @10d9e | 18412583 | 4-msg | `fastpbs` | [0002](history/entries/0002--10d9e.md) | hand radix-4 SIMD FFT, split spectra, branchless decompose, fully-SIMD FFT scalar passes |
| 0003 | 2026-06-29 | @10d9e | 14878208 | **boolean** | `boolgate` | [0003](history/entries/0003--10d9e.md) | **boolean gate** at n=700 — beats rs-tfhe (~14.4 vs 14.7 ms, same machine/params; faster hand FFT) |
