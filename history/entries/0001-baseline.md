# Entry 0001 — SCORE 12023834 (-2162207 (new record))

| Field | Value |
|-------|-------|
| Date | 2026-06-28 |
| Author | @10d9e |
| Model | — |
| Git author | autoresearch |
| Commit | `f59fe38` |
| SCORE | 12023834 |
| Δ vs previous record | -2162207 (new record) |
| Status | record |

## Approach

Tune the blind-rotation length from n=600 to **n=500** (the classic TFHE gate-bootstrap
dimension). The bootstrap cost is dominated by the n CMux external products, so shortening
the blind rotation is the largest single lever: ~14 ms → ~12 ms. Same optimized f64-FFT
transform (folded N/2 FFT, reusable buffers, precomputed twiddle tables).

## Algorithm changes

```
(harness parameter: blind-rotation length n 600 -> 500)
```

## Eval snapshot

```
fixture           got want    margin  ok
identity/m0         0    0    14.1b  OK
increment/m0        1    1    14.6b  OK
identity/m1         1    1    16.8b  OK
increment/m1        2    2    12.7b  OK
identity/m2         2    2    15.5b  OK
increment/m2        3    3    12.9b  OK
identity/m3         3    3    13.7b  OK
increment/m3        0    0    12.6b  OK
--------------------------------------------

SCORE: 11546042 ns/bootstrap  (median of 31; best 10993000 ns) — LOWER IS BETTER
       = 11.546 ms
```
