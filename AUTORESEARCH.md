# Autoresearch rules — bootstrap

## The problem

Implement the **fastest** TFHE programmable bootstrap that is **≥128-bit secure**. A
bootstrap takes a noisy LWE ciphertext of a message `m` and a LUT `f`, and returns a fresh
LWE ciphertext of `f(m)` with reduced noise, over `q = 2^64`.

Unlike a fixed-parameter benchmark, **you choose the parameters.** Any `n`, `k`, `N`,
decomposition, and noise are allowed as long as they clear the security gate and bootstrap
correctly.

## What you may change

**Only `src/algorithm/**`.** Everything else is frozen: the LWE encode/decode, the
secret/encryption oracle, the security estimator, the fixtures, and the scoring. The
boundary is enforced by `scripts/guard.sh` (and CI).

## The contract (do not change these signatures)

```rust
pub struct ServerKey;
pub fn params() -> Params;                         // your parameter choice
pub fn keygen(sk: &SecretKey, seed: u64) -> ServerKey;   // untimed
pub fn bootstrap(sk: &ServerKey, ct: &Lwe, lut: &Lut) -> Lwe;   // timed
```

`params()` declares your parameter set. `keygen` builds the public bootstrap material
(picking its own internal GLWE key) and is **not** timed. `bootstrap` is the **timed**
operation.

## Gates (all must pass, or the candidate is INVALID)

1. **Functional spec** — `params().message_bits == 3` (a 4-message space). You can't shrink
   the message space to game the score.
2. **Security ≥128 bits** — both the input/output LWE (dimension `n`) and the GLWE
   (dimension `k·N`) must reach **≥128-bit classical core-SVP** security at `q = 2^64` with
   binary secret keys, per the built-in estimator (`src/harness/security.rs`).
3. **Correctness** — every fixture (each message under several LUTs, including non-identity
   ones a pass-through could not satisfy) decodes to `f(message)` with a comfortable noise
   margin (a genuine, noise-reducing refresh). `cargo test --release` checks this on
   off-corpus inputs.

## The security estimator

`src/harness/security.rs` is a self-contained Rust approximation of the
[lattice-estimator](https://github.com/malb/lattice-estimator): the **primal-uSVP** attack
under the **classical core-SVP** cost model `2^{0.292·β}`, minimized over BKZ block size and
samples, with the secret rescaled to the error scale (so binary secrets are accounted for).
It is **conservative** — core-SVP lower-bounds the real attack cost, so clearing it implies
the full estimator reports at least as much. (Calibration: Kyber512 → ~115 core-SVP bits vs
the published ~118.) For final sign-off, cross-check with the actual lattice-estimator.

To raise security: increase the dimension (`n`, or `k·N`) and/or the noise (`lwe_sigma`,
`glwe_sigma`). Larger noise must still bootstrap correctly — the input noise is rounded away
by the mod-switch, but the key-switch / bootstrap-key noise lands in the output, so there is
a window between the security floor and the message gap.

## Score

**SCORE = median wall-clock nanoseconds for one `bootstrap`, lower is better.** `keygen`
time does not count. Timing is machine-dependent — the official winner is determined on a
**fixed reference runner**.

## Ideas

- Real native SIMD (AVX2/AVX-512, NEON) or a hand-tuned FFT vs the generic `rustfft`.
- An exact NTT if it beats the f64 transform on this CPU.
- Find a *cheaper* secure parameter corner: smaller `n` with larger `lwe_sigma`, a different
  `(k, N)` split, fewer decomposition levels — anything that clears the gate and bootstraps.
- Deeper register/buffer tiling, batched transforms, parallel CMux.
