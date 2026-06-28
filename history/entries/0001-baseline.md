# Entry 0001 — SCORE 12023834 (baseline)

| Field | Value |
|-------|-------|
| Date | 2026-06-28 |
| Author | @10d9e |
| Model | — |
| Git author | autoresearch |
| Commit | `baseline` |
| SCORE | 12023834 |
| Δ vs previous record | — (initial baseline) |
| Status | record |

## Approach

Baseline autoresearch harness with a TFHE programmable bootstrap (CGGI blind rotation +
sample-extract + key-switch) over an approximate f64 complex-FFT negacyclic multiplier
(folded length-N/2 FFT). Parameters: N=1024, k=1, n=500, pbs ℓ=2 baselog=12, ks ℓ=5.
`keygen` builds the GGSW bootstrap key + key-switch key and is untimed.

## Algorithm changes

```
(none — starting point)
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

SCORE: 12023834 ns/bootstrap  (median of 31; best 11746584 ns) — LOWER IS BETTER
       = 12.024 ms
```
