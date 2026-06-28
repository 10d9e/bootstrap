# Entry 0000 — SCORE 49499417 (baseline)

| Field | Value |
|-------|-------|
| Date | 2026-06-28 |
| Author | @10d9e |
| Model | — |
| Git author | autoresearch |
| Commit | `secure` |
| SCORE | 49499417 |
| Δ vs previous record | — (initial baseline) |
| Status | record |

## Approach

First **≥128-bit-secure** baseline. A TFHE programmable bootstrap (CGGI blind rotation +
sample-extract + key-switch) over an approximate f64 complex-FFT negacyclic multiplier
(folded length-N/2 FFT). Parameters chosen to clear the harness's classical core-SVP gate:

- input/output LWE: `n=1024`, `σ=2^44` → **130.8-bit** core-SVP.
- GLWE: `k=1, N=2048` (dim 2048), `σ=2^29` → **163.8-bit** core-SVP.
- `pbs ℓ=2 baselog=12`, `ks ℓ=5 baselog=4`, 4-message space.

The large LWE noise needed for security is rounded away by the mod-switch on input; the
key-switch / bootstrap-key noise lands in the output, leaving a comfortable margin
(~7-9 bits) below the message gap.

## Algorithm changes

```
(none — starting point)
```

## Eval snapshot

```
params: n=1024 k=1 N=2048 | LWE σ=2^44.0 dim 1024 → 130.8 bits | GLWE σ=2^29.0 dim 2048 → 163.8 bits
security gate: OK (≥128 core-SVP bits)
fixture           got want    margin  ok
identity/m0         0    0     9.4b  OK
increment/m0        1    1     9.4b  OK
identity/m1         1    1     6.8b  OK
increment/m1        2    2     6.6b  OK
identity/m2         2    2     7.6b  OK
increment/m2        3    3     7.7b  OK
identity/m3         3    3     8.9b  OK
increment/m3        0    0     9.4b  OK
--------------------------------------------

SCORE: 49499417 ns/bootstrap  (median of 21; best 48340084 ns) — LOWER IS BETTER
       = 49.499 ms
```
