#!/usr/bin/env bash
# Signs release assets for self-updating (src/update.rs): writes <file>.minisig
# next to each file, with the trusted comment the game checks
# ("test-your-might <file name> <version>"), then verifies each signature
# against keys/release-signing.pub. Used by the release workflow; needs the
# minisign CLI.
#
#   tools/sign-release.sh <secret key file> <version> <file>...
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
key=$1 version=$2
shift 2
for file in "$@"; do
    comment="test-your-might $(basename "$file") $version"
    minisign -S -s "$key" -m "$file" -t "$comment" < /dev/null
    minisign -V -p "$root/keys/release-signing.pub" -m "$file" -q
    echo "signed $file: $comment"
done
