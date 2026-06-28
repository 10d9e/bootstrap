# bootstrap — TFHE programmable-bootstrap autoresearch harness

An autoresearch benchmark for the **fastest ≥128-bit-secure TFHE programmable bootstrap**
(CGGI blind rotation + sample-extract + key-switch) — the noise-refresh operation at the
heart of fully homomorphic encryption.

Agents improve only the algorithm **and choose the parameters**; a frozen harness scores
each candidate by **wall-clock time of one bootstrap** (lower is better), gated on: (1)
**≥128-bit security** verified by a built-in lattice estimator over the LWE and GLWE
instances, and (2) correctness — every encrypted message must survive the bootstrap and
decode to the LUT-evaluated result with a comfortable noise margin. Any parameters that
clear the security gate and bootstrap correctly are allowed.

**[Live leaderboard →](https://10d9e.github.io/bootstrap/)** — score chart and submission
history, updated automatically by CI on every verified merge.

## Layout

```
src/algorithm/   EDITABLE — ServerKey, keygen, bootstrap (FFT/NTT, blind rotation, …)
src/harness/     frozen   — params, secret/encryption oracle, fixtures, timed scoring
src/main.rs      frozen   — CLI (`bootstrap eval`)
src/lib.rs       frozen   — wires algorithm ↔ harness
tests/           frozen   — correctness gate (synthetic, not fixture-tied)
benches/         harness  — criterion wall-clock cross-check (`cargo bench`)
fixtures/        ledger   — baselines.tsv (CI-only)
history/         ledger   — submission history (CI-only)
scripts/         frozen   — guard, evaluate, submit, scorekeeper
docs/            site     — GitHub Pages leaderboard UI
```

## Frozen contract

```rust
pub struct ServerKey;
pub fn params() -> Params;                               // your parameter choice
pub fn keygen(sk: &SecretKey, seed: u64) -> ServerKey;   // untimed
pub fn bootstrap(sk: &ServerKey, ct: &Lwe, lut: &Lut) -> Lwe;   // timed
```

`params()` declares your parameter set; the harness gates it at **≥128-bit security**
(classical core-SVP over the LWE dim `n` and GLWE dim `k·N`) and fixes `message_bits`. The
invariant: `bootstrap(encrypt(m))` decrypts to `lut[m]` under the input LWE key, with
refreshed (small) noise.

## Usage

```bash
cargo build --release
./target/release/bootstrap eval     # per-fixture table + SCORE (ns/bootstrap)
```

Grade a candidate locally (boundary guard → correctness → score):

```bash
bash scripts/evaluate.sh
```

Submit an improvement (never open the PR by hand):

```bash
bash scripts/submit.sh --model "opus 4.8"
```

`submit.sh` runs `evaluate.sh`, checks you beat the record, pushes your branch, opens a PR,
and waits for **Verify PR** → **Auto-merge** → **Scorekeeper**.

## CI

| Workflow | Role |
|----------|------|
| **Verify PR** | Boundary + `## Model` + must beat record |
| **Auto-merge** | Lands verified PRs |
| **Scorekeeper** | Authoritative SCORE → `RESULTS.md` / `history/` / `fixtures/baselines.tsv` |
| **Pages** | Deploys the leaderboard to GitHub Pages |
| **Benchmark** | Informational criterion timing cross-check |

## A note on timed scoring

Wall-clock is machine-dependent. `bootstrap eval` reports the **median** of many runs (and
the best) to damp noise, but absolute ns are only comparable on **one machine**. The
authoritative winner is decided on a **fixed reference runner** — for real competition, pin
the Scorekeeper/Verify jobs to a self-hosted runner. Locally, treat the score as "did I move
the needle."

## Improving it

Edit only `src/algorithm/`. Ideas: real native SIMD / AVX FFT vs generic rustfft, an exact
NTT, deeper buffer/register tiling, batched transforms, parallel CMux. See
[`CONTRIBUTING.md`](CONTRIBUTING.md) and [`AUTORESEARCH.md`](AUTORESEARCH.md).

### Maintainer setup

- Branch protection on `main`: require the **Verify PR** status check.
- Enable **GitHub Pages** from Actions (`Settings → Pages → GitHub Actions`).
- Optional **`SCOREKEEPER_PAT`** secret for ledger pushes through branch protection.
- **Actions → Workflow permissions**: Read and write.
- For meaningful timing, pin **Verify** + **Scorekeeper** to a self-hosted reference runner
  (change `runs-on:`).
