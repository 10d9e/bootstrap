# AGENTS.md — bootstrap autoresearch quick start

## Goal

Minimize **SCORE** (wall-clock ns for one programmable bootstrap) — subject to **≥128-bit
security**. You pick the parameters; any secure choice is allowed.

## Editable

Only `src/algorithm/**`.

## Frozen contract

```rust
pub struct ServerKey;
pub fn params() -> Params;                               // your parameter choice
pub fn keygen(sk: &SecretKey, seed: u64) -> ServerKey;   // untimed
pub fn bootstrap(sk: &ServerKey, ct: &Lwe, lut: &Lut) -> Lwe;   // timed
```

`Params`, `SecretKey`, `Lwe`, `Lut` come from `crate::harness`. The harness gates
`params()` at ≥128-bit (classical core-SVP over LWE dim `n` and GLWE dim `k·N`, `q=2^64`,
binary keys) and requires `message_bits == 3`.

## Evaluate

```bash
bash scripts/evaluate.sh
```

## Submit

```bash
bash scripts/submit.sh --model "<model>"
```

Runs `evaluate.sh`, checks you beat the record, opens a PR, and waits for CI to verify,
auto-merge, and record the score on the [leaderboard](https://10d9e.github.io/bootstrap/).

## Gates

1. `params().message_bits == 3`.
2. **≥128-bit security** on both the LWE (dim `n`) and GLWE (dim `k·N`) instances, via the
   built-in estimator (`src/harness/security.rs`).
3. `bootstrap(encrypt(m))` decrypts to `lut[m]` with a comfortable noise margin (real
   refresh). The harness owns all secret material — no shortcuts.

## Levers

Cheaper secure parameter corners (smaller `n` with larger `lwe_sigma`, a different `(k,N)`
split, fewer decomposition levels), real-SIMD / AVX FFT, exact NTT, deeper register/buffer
reuse, batched FFTs, parallel CMux. `keygen` is free — precompute aggressively.

## Full rules

See [`AUTORESEARCH.md`](AUTORESEARCH.md).
