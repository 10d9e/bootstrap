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
5. Submit with one script:
   ```bash
   bash scripts/submit.sh --model "opus 4.8"
   ```
   It checks `gh` auth, runs `evaluate.sh`, verifies you beat the record, commits algorithm
   changes, opens a PR with **`## Model`** / **`## Approach`**, and waits for CI to verify,
   auto-merge, and record the score.

## Live leaderboard

After enabling GitHub Pages, the site lives at **https://10d9e.github.io/bootstrap/**
(rebuilds automatically when Scorekeeper updates `RESULTS.md`).

## CI is the source of truth

| What | Who updates it |
|------|----------------|
| `src/algorithm/` | You (via PR) |
| `RESULTS.md`, `history/entries/`, `fixtures/baselines.tsv` | **Scorekeeper CI only** |
| Leaderboard site (`docs/data/leaderboard.json`) | **Pages CI** from the ledger |

**Do not** commit ledger files in your PR.

## Pull request checklist

- [ ] Only `src/algorithm/` changed
- [ ] PR has `## Model` and `## Approach`
- [ ] SCORE beats the current record on the reference runner
- [ ] No fixture-specific tuning or side channels

## How merges are gated

1. **Verify PR** — boundary guard, `## Model` required, score gate (must beat record)
2. **Auto-merge** — lands passing PRs to `main`
3. **Scorekeeper** — authoritative `evaluate.sh`, appends ledger + history entry
4. **Pages** — rebuilds the leaderboard site

## Notes on timed scoring

Wall-clock varies by machine, thermal state, and load. Build with `--release`; prefer the
**median** the eval reports; and remember the absolute ns are only comparable on the
reference runner. A local improvement should be robust (helps across several runs), not
within noise.

### Maintainer setup

- Branch protection on `main`: require **Verify PR**.
- Enable **GitHub Pages** from Actions.
- Optional **`SCOREKEEPER_PAT`** secret for ledger pushes through branch protection.
- **Actions → Workflow permissions**: Read and write.
- Pin **Verify** + **Scorekeeper** to a self-hosted reference runner for stable timing.

## Questions

Open an issue for harness bugs. Algorithm ideas belong in PRs.
