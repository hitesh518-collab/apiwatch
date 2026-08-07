#!/usr/bin/env bash
# Verify the SHA-256 checksum of a release archive.
#
# Usage: verify_checksum.sh <archive> <checksums-file> <expected-filename>
#
# Exit codes:
#   0  checksum matches
#   1  checksum mismatch
#   2  input error (missing file, missing entry, malformed data)
set -euo pipefail

ARCHIVE="$1"
SUMS="$2"
FILENAME="$3"

if [ ! -f "$ARCHIVE" ]; then
    echo "::error::archive not found: $ARCHIVE"
    exit 2
fi

if [ ! -f "$SUMS" ]; then
    echo "::error::checksum file not found: $SUMS"
    exit 2
fi

EXPECTED=$(grep -F " $FILENAME" "$SUMS" | awk '{print $1}' || true)
if [ -z "$EXPECTED" ]; then
    echo "::error::$FILENAME not found in $SUMS"
    exit 2
fi

if ! echo "$EXPECTED" | grep -qEx '[0-9a-f]{64}'; then
    echo "::error::malformed checksum entry for $FILENAME"
    exit 2
fi

if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL=$(sha256sum "$ARCHIVE" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
    ACTUAL=$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')
else
    echo "::error::no SHA-256 tool available (sha256sum or shasum required)"
    exit 2
fi

if [ "$ACTUAL" != "$EXPECTED" ]; then
    echo "::error::checksum mismatch for $FILENAME"
    echo "  expected: $EXPECTED"
    echo "  actual:   $ACTUAL"
    exit 1
fi

echo "checksum verified: $FILENAME"
exit 0
