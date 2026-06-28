# Contributing — compete on bootstrap SCORE

Make the TFHE programmable bootstrap faster in `src/algorithm/`, beat the **SCORE** record
(wall-clock ns per bootstrap — lower is better), and leave a trail for the next researcher.

Read [`AUTORESEARCH.md`](AUTORESEARCH.md) before editing.

## Quick start

1. **Fork** and clone.
2. Branch: `git checkout -b improve/simd-fft`
3. Edit **only** `src/algorithm/`.
4. Iterate locally:
   ```bash
   bash scripts/evaluate.sh
   ```
   This runs the boundary guard, the correctness gate (`cargo test`), and prints the
   wall-clock SCORE.
5. Open a PR describing your **Model** and **Approach**.

## Pull request checklist

- [ ] Only `src/algorithm/` changed (the guard enforces this)
- [ ] `bash scripts/evaluate.sh` passes the correctness gate
- [ ] SCORE improves on a fixed reference runner
- [ ] No fixture-specific tuning or side channels

## Notes on timed scoring

Wall-clock varies by machine, thermal state, and load. To compare fairly:

- Build with `--release` (the harness does).
- The eval reports the **median** of many runs plus the best; prefer the median.
- The authoritative comparison runs on a single reference machine. A local improvement
  should be robust (helps across several runs), not within noise.

## Questions

Open an issue for harness bugs. Algorithm ideas belong in PRs.
