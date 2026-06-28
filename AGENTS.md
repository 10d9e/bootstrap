# AGENTS.md — bootstrap autoresearch quick start

## Goal

Minimize **SCORE** (wall-clock ns for one programmable bootstrap) at `N=1024`, `k=1`.

## Editable

Only `src/algorithm/**`.

## Frozen contract

```rust
pub struct ServerKey;
pub fn keygen(params: Params, sk: &SecretKey, seed: u64) -> ServerKey;   // untimed
pub fn bootstrap(sk: &ServerKey, ct: &Lwe, lut: &Lut) -> Lwe;            // timed
```

`Params`, `SecretKey`, `Lwe`, `Lut` come from `crate::harness`.

## Evaluate

```bash
bash scripts/evaluate.sh
```

## Invariant

`bootstrap(encrypt(m))` decrypts (under the input LWE key) to `lut[m]`, with a comfortable
noise margin (a real refresh). The harness owns all secret material — you only see public
keys and ciphertexts, so you cannot pass without a genuine bootstrap.

## Levers

Real-SIMD / AVX FFT, exact NTT, deeper register/buffer reuse, batched FFTs, better gadget
decomposition, parallelism across CMux/primes. `keygen` is free — precompute aggressively.

## Full rules

See [`AUTORESEARCH.md`](AUTORESEARCH.md).
