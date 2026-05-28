#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 4 ]]; then
  echo "Usage: $0 <version> <tag> <signature-file> <tarball-filename>" >&2
  echo "Example: $0 0.2.0 v0.2.0 target/release/bundle/macos/TimeTrack.app.tar.gz.sig TimeTrack.app.tar.gz" >&2
  exit 1
fi

VERSION="$1"
TAG="$2"
SIG_FILE="$3"
TARBALL="$4"
OUT="${5:-latest.json}"

if [[ ! -f "$SIG_FILE" ]]; then
  echo "Signature file not found: $SIG_FILE" >&2
  exit 1
fi

SIGNATURE="$(tr -d '\n' < "$SIG_FILE")"
PUB_DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
BASE_URL="https://github.com/Dr0nhp/timetrack/releases/download/${TAG}/${TARBALL}"

cat > "$OUT" <<EOF
{
  "version": "${VERSION}",
  "notes": "TimeTrack ${VERSION}",
  "pub_date": "${PUB_DATE}",
  "platforms": {
    "darwin-aarch64": {
      "signature": "${SIGNATURE}",
      "url": "${BASE_URL}"
    }
  }
}
EOF

echo "Wrote ${OUT} for version ${VERSION}"
