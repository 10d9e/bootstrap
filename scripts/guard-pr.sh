#!/usr/bin/env bash
# PR boundary guard: algorithm submissions may change ONLY src/algorithm/.
# Site PRs may change ONLY docs/ or scripts/build-leaderboard.py.
# Infra PRs may change ONLY .github/ or scripts/ (not combined with algorithm).
# FROZEN — do not edit as part of autoresearch.
set -euo pipefail
cd "$(dirname "$0")/.."

base="${1:-}"
if [[ -z "$base" ]]; then
  if [[ -n "${GITHUB_BASE_SHA:-}" ]]; then
    base="$GITHUB_BASE_SHA"
  elif git rev-parse origin/main >/dev/null 2>&1; then
    base="$(git merge-base HEAD origin/main)"
  else
    base="$(git rev-parse HEAD~1)"
  fi
fi

violations=()
has_algorithm=0
has_infra=0
while IFS= read -r f; do
  [[ -z "$f" ]] && continue
  case "$f" in
    src/algorithm/*) has_algorithm=1 ;;
    docs/*|scripts/build-leaderboard.py) ;;
    .github/*|scripts/*) has_infra=1 ;;
    *) violations+=("$f") ;;
  esac
done < <(git diff --name-only "$base"...HEAD)

if (( ${#violations[@]} )); then
  echo "PR BOUNDARY VIOLATION — algorithm submissions may only change src/algorithm/;"
  echo "site PRs may only change docs/ or scripts/build-leaderboard.py;"
  echo "infra PRs may only change .github/ or scripts/:"
  printf '  %s\n' "${violations[@]}"
  echo
  echo "Do not commit RESULTS.md, history/entries/, or fixtures/baselines.tsv — CI records on merge."
  exit 1
fi

if (( has_algorithm && has_infra )); then
  echo "PR BOUNDARY VIOLATION — infra changes (.github/ or scripts/) may not be"
  echo "combined with a src/algorithm submission; submit them as separate PRs."
  exit 1
fi

mod=src/algorithm/mod.rs
if (( has_algorithm )); then
  if ! grep -qF 'pub struct ServerKey' "$mod" \
    || ! grep -qF 'pub fn params() -> Params' "$mod" \
    || ! grep -qF 'pub fn keygen(sk: &SecretKey, seed: u64) -> ServerKey' "$mod" \
    || ! grep -qF 'pub fn bootstrap(sk: &ServerKey, ct: &Lwe, lut: &Lut) -> Lwe' "$mod"; then
    echo "PR BOUNDARY VIOLATION — frozen ServerKey/params/keygen/bootstrap signatures were changed."
    exit 1
  fi
  if grep -rqE '#\[\s*global_allocator\s*\]' src/algorithm/ 2>/dev/null; then
    echo "PR BOUNDARY VIOLATION — src/algorithm/ must not declare a #[global_allocator]"
    exit 1
  fi
  echo "PR boundary OK (only src/algorithm/ changed; contract intact)"
elif (( has_infra )); then
  echo "PR boundary OK (infra changes only)"
else
  echo "PR boundary OK (site or no algorithm changes)"
fi
