#!/usr/bin/env bash
# Reproducible terminal demo for APIWatch's hero workflow.
#
# Runs the observed JSON drift demo and produces output suitable for
# terminal recording tools (VHS, asciinema, script).
#
# Usage:
#   bash scripts/demo.sh [--binary ./target/release/apiwatch]
#
# The demo is deterministic and uses committed fixtures only.

set -euo pipefail

BINARY="${APIWATCH_BINARY:-./target/release/apiwatch}"
if [ -n "${1:-}" ] && [ "$1" = "--binary" ]; then
    BINARY="${2:-$BINARY}"
fi

DEMO_DIR="examples/observed-json-drift"
LOCK="$DEMO_DIR/api.lock"

echo "# APIWatch — Observed Contract Demo"
echo ""
echo "# 1. Record the expected structure from a sample response"
echo "$ $BINARY record --from-json $DEMO_DIR/baseline.json --name payments --output $LOCK"
"$BINARY" record --from-json "$DEMO_DIR/baseline.json" --name payments --output "$LOCK"

echo ""
echo "# 2. Verify the same structure — passes"
echo "$ $BINARY verify $DEMO_DIR/baseline.json --name payments --lock $LOCK"
"$BINARY" verify "$DEMO_DIR/baseline.json" --name payments --lock "$LOCK"

echo ""
echo "# 3. Verify a breaking change — amount changed from number to string"
echo "$ $BINARY verify $DEMO_DIR/changed.json --name payments --lock $LOCK"
set +e
"$BINARY" verify "$DEMO_DIR/changed.json" --name payments --lock "$LOCK"
EXIT_CODE=$?
set -e

echo ""
echo "# 4. Inspect the lockfile — no captured values"
echo "$ grep -E '(42.50|pay_123|USD|complete)' $LOCK"
grep -E '(42\.50|pay_123|USD|complete)' "$LOCK" && echo "(values found — BUG)" || echo "(no captured values)"

echo ""
echo "# Exit code for breaking change: $EXIT_CODE (1 = breaking drift)"
echo "# Done. The lockfile stores structure only and can be committed to Git."
