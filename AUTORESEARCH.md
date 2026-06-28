# Autoresearch rules — bootstrap

## The problem

Implement the fastest correct **TFHE programmable bootstrap** at the fixed parameters in
`src/harness/params.rs` (`N=1024`, `k=1`, `n=500`, `pbs_l=2`, `pbs_baselog=12`, `ks_l=5`,
`ks_baselog=4`, 4-message space, `q=2^64`).

A bootstrap takes a noisy LWE ciphertext of a message `m` and a LUT `f`, and returns a fresh
LWE ciphertext of `f(m)` with reduced noise.

## What you may change

**Only `src/algorithm/**`.** Everything else is frozen: the parameters, the LWE
encode/decode, the secret/encryption oracle, the fixtures, and the scoring. The boundary is
enforced by `scripts/guard.sh` (and CI).

## The contract (do not change these signatures)

```rust
pub struct ServerKey;
pub fn keygen(params: Params, sk: &SecretKey, seed: u64) -> ServerKey;
pub fn bootstrap(sk: &ServerKey, ct: &Lwe, lut: &Lut) -> Lwe;
```

- `keygen` builds the public bootstrap material from the secret LWE key (it chooses its own
  internal GLWE key). It runs **once** and is **not** part of the score — precompute freely.
- `bootstrap` is the **timed** operation.

## Correctness gate (must pass, or the candidate is INVALID)

For every fixture (each message under several LUTs, including non-identity ones a
pass-through could not satisfy):

1. `decrypt(bootstrap(encrypt(m)))` equals `lut[m]`, **and**
2. the output noise sits a comfortable margin below the decode boundary (a real refresh).

`cargo test --release` runs the gate on synthetic inputs (distinct from the scored
fixtures), so overfitting to the fixtures fails the tests.

## Score

**SCORE = wall-clock nanoseconds for one `bootstrap`, lower is better.** `bootstrap eval`
reports the **median** over many runs (plus the best) after warm-up. `keygen` time does not
count.

Timing is machine-dependent: the official winner is determined on a **fixed reference
runner**. Locally, the score tells you whether a change helped.

## Anti-gaming

- The harness holds the secret key; you only ever touch public material and ciphertexts, so
  you cannot shortcut the bootstrap.
- No fixture-specific tuning or side channels (the tests use off-corpus inputs).
- `src/algorithm/` must not declare a `#[global_allocator]`.

## Ideas

- Real native SIMD (AVX2/AVX-512, NEON) or a hand-tuned FFT vs the generic `rustfft`.
- An exact NTT (no FFT dependency) if it beats the f64 transform on this CPU.
- Deeper register/buffer tiling, batched transforms, fewer memory sweeps.
- Better gadget decomposition; fusing sample-extract/key-switch.
- Parallelism across the `n` CMux external products.
