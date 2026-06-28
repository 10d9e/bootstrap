# bootstrap — TFHE programmable-bootstrap autoresearch harness

An autoresearch benchmark for the **TFHE programmable bootstrap** (CGGI blind rotation +
sample-extract + key-switch) at `N=1024`, `k=1` — the noise-refresh operation at the heart
of fully homomorphic encryption.

Agents improve only the algorithm; a frozen harness scores each candidate by **wall-clock
time of one bootstrap** (lower is better), gated on a correctness oracle: every encrypted
message must survive the bootstrap and decode to the LUT-evaluated result with a comfortable
noise margin (a genuine refresh, not a pass-through).

## Layout

```
src/algorithm/   EDITABLE — ServerKey, keygen, bootstrap (FFT/NTT, blind rotation, …)
src/harness/     frozen   — params, secret/encryption oracle, fixtures, timed scoring
src/main.rs      frozen   — CLI (`bootstrap eval`)
src/lib.rs       frozen   — wires algorithm ↔ harness
tests/           frozen   — correctness gate (synthetic, not fixture-tied)
benches/         harness  — criterion wall-clock cross-check (`cargo bench`)
scripts/         frozen   — boundary guard + evaluate
```

## Frozen contract

```rust
pub struct ServerKey;
pub fn keygen(params: Params, sk: &SecretKey, seed: u64) -> ServerKey;
pub fn bootstrap(sk: &ServerKey, ct: &Lwe, lut: &Lut) -> Lwe;
```

`keygen` builds the public bootstrap material from the secret key and is **not timed**.
`bootstrap` is the timed operation. The invariant: `bootstrap(encrypt(m))` must decrypt to
`lut[m]` under the input LWE key, with refreshed (small) noise.

## Usage

```bash
cargo build --release
./target/release/bootstrap eval     # prints the per-fixture table + SCORE (ns/bootstrap)
```

Grade a candidate locally (boundary guard → correctness → score):

```bash
bash scripts/evaluate.sh
```

Detailed timing distribution:

```bash
cargo bench
```

## Improving it

Edit **only** `src/algorithm/`, then `bash scripts/evaluate.sh`. Ideas: faster/real-SIMD
FFT, an exact NTT, deeper buffer reuse, better decomposition, parallelism. See
[`AUTORESEARCH.md`](AUTORESEARCH.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md).

## A note on timed scoring

Wall-clock is machine-dependent. The eval reports the **median** of many runs (and the best)
to damp noise; the authoritative winner is decided on a **fixed reference runner**. Locally,
treat the score as "did I move the needle," not an absolute.
