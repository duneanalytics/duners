#!/usr/bin/env bash
# Ensures doc coverage is at or above the baseline (documented %, examples %).
# Update scripts/doc-coverage-baseline.txt when you intentionally improve coverage.
#
# Test locally (requires nightly: rustup toolchain install nightly):
#   bash scripts/check-doc-coverage.sh
#
# Test parsing only (skip running cargo):
#   DOC_COVERAGE_OUTPUT=scripts/fixtures/doc-coverage-sample.txt bash scripts/check-doc-coverage.sh   # should pass
#   DOC_COVERAGE_OUTPUT=scripts/fixtures/doc-coverage-below-baseline.txt bash scripts/check-doc-coverage.sh   # should fail

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

BASELINE_FILE="$REPO_ROOT/scripts/doc-coverage-baseline.txt"
if [[ ! -f "$BASELINE_FILE" ]]; then
  echo "Missing baseline file: $BASELINE_FILE"
  exit 1
fi

read -r MIN_DOC MIN_EX < "$BASELINE_FILE"

if [[ -n "${DOC_COVERAGE_OUTPUT:-}" ]]; then
  echo "Using existing coverage output: $DOC_COVERAGE_OUTPUT"
  COVERAGE_OUTPUT="$DOC_COVERAGE_OUTPUT"
else
  echo "Running rustdoc coverage (nightly)..."
  COVERAGE_OUTPUT=$(mktemp)
  trap 'rm -f "$COVERAGE_OUTPUT"' EXIT
  cargo +nightly rustdoc -Z unstable-options -- -Z unstable-options --show-coverage 2>&1 | tee "$COVERAGE_OUTPUT"
fi

TOTAL_LINE=$(grep '| Total ' "$COVERAGE_OUTPUT" || true)
if [[ -z "$TOTAL_LINE" ]]; then
  echo "Could not find '| Total' in rustdoc output"
  exit 1
fi

# Parse documented % and examples % from the table (columns 4 and 6 after splitting on |)
CURRENT_DOC=$(echo "$TOTAL_LINE" | awk -F'|' '{ gsub(/[ %]/,"",$4); print $4 }')
CURRENT_EX=$(echo "$TOTAL_LINE" | awk -F'|' '{ gsub(/[ %]/,"",$6); print $6 }')

echo ""
echo "Baseline: documented >= $MIN_DOC%, examples >= $MIN_EX%"
echo "Current:  documented = ${CURRENT_DOC}%, examples = ${CURRENT_EX}%"

FAIL=0
if awk -v a="$CURRENT_DOC" -v b="$MIN_DOC" 'BEGIN{exit (a+0>=b+0)?0:1}'; then
  :
else
  echo "FAIL: documented coverage decreased (${CURRENT_DOC}% < ${MIN_DOC}%)"
  FAIL=1
fi
if awk -v a="$CURRENT_EX" -v b="$MIN_EX" 'BEGIN{exit (a+0>=b+0)?0:1}'; then
  :
else
  echo "FAIL: examples coverage decreased (${CURRENT_EX}% < ${MIN_EX}%)"
  FAIL=1
fi

if [[ $FAIL -eq 1 ]]; then
  echo "Update scripts/doc-coverage-baseline.txt if this decrease is intentional."
  exit 1
fi

echo "Doc coverage check passed."
