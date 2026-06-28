#!/usr/bin/env bash
# Evaluate one candidate: boundary guard -> correctness gate -> wall-clock score.
# FROZEN — do not edit as part of autoresearch.
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ "${1:-}" != "--no-guard" ]]; then
  echo "== boundary guard =="
  bash scripts/guard.sh
fi

echo "== correctness gate (cargo test) =="
if ! cargo test --release >/tmp/bootstrap_test.log 2>&1; then
  echo "TESTS FAILED — candidate is INVALID:"
  tail -n 30 /tmp/bootstrap_test.log
  exit 1
fi
grep -E "test result" /tmp/bootstrap_test.log

echo "== build =="
cargo build --release --quiet

echo "== score (wall-clock; lower is better) =="
./target/release/bootstrap eval
