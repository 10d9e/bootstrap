#!/usr/bin/env bash
# Local boundary guard: fail if anything outside src/algorithm/ changed vs HEAD, or if the
# frozen ServerKey/keygen/bootstrap contract was altered.
# FROZEN — do not edit as part of autoresearch.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! git rev-parse --git-dir >/dev/null 2>&1; then
  echo "guard: not a git repo; run 'git init && git add -A && git commit -m base' first" >&2
  exit 2
fi

violations=()
while IFS= read -r f; do
  [[ -z "$f" ]] && continue
  case "$f" in
    src/algorithm/*) ;;
    *) violations+=("$f") ;;
  esac
done < <( { git diff --name-only HEAD; git ls-files --others --exclude-standard; } | sort -u )

if (( ${#violations[@]} )); then
  echo "BOUNDARY VIOLATION — these frozen files were modified:"
  printf '  %s\n' "${violations[@]}"
  echo "Only src/algorithm/ may change locally."
  exit 1
fi

mod=src/algorithm/mod.rs
if ! grep -qF 'pub struct ServerKey' "$mod" \
  || ! grep -qF 'pub fn keygen(params: Params, sk: &SecretKey, seed: u64) -> ServerKey' "$mod" \
  || ! grep -qF 'pub fn bootstrap(sk: &ServerKey, ct: &Lwe, lut: &Lut) -> Lwe' "$mod"; then
  echo "BOUNDARY VIOLATION — frozen ServerKey/keygen/bootstrap signatures were changed."
  exit 1
fi

if grep -rqE '#\[\s*global_allocator\s*\]' src/algorithm/ 2>/dev/null; then
  echo "BOUNDARY VIOLATION — src/algorithm/ must not declare a #[global_allocator]"
  exit 1
fi

echo "boundary OK (only src/algorithm/ changed; contract intact)"
