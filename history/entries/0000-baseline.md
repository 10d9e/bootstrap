# Entry 0000 — SCORE 14186041 (baseline)

| Field | Value |
|-------|-------|
| Date | 2026-06-28 |
| Author | @10d9e |
| Model | — |
| Git author | autoresearch |
| Commit | `f59fe38` |
| SCORE | 14186041 |
| Δ vs previous record | — (initial baseline) |
| Status | record |

## Approach

Original baseline, before tuning the blind-rotation length. A TFHE programmable bootstrap
(CGGI blind rotation + sample-extract + key-switch) over an approximate f64 complex-FFT
negacyclic multiplier (folded length-N/2 FFT), at blind-rotation length **n=600**. The
dominant cost is the n CMux external products, so this sets the pre-tuning reference (~14 ms,
the "15 ms baseline").

## Algorithm changes

```
(none — starting point)
```

## Eval snapshot

```
all fixtures OK (identity + increment LUTs, every message, comfortable noise margin)
SCORE: 14186041 ns/bootstrap  (median of 31) — LOWER IS BETTER
       = 14.186 ms
```
