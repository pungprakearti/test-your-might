#!/usr/bin/env bash
# Packages a `trunk build --release` (dist/) for `vercel deploy --prebuilt`,
# in Vercel's Build Output API layout, so Vercel serves the files as they are
# and never builds anything itself (it has no Rust). Trunk puts a content hash
# in every file name except index.html, so those are cached for good and
# index.html (Vercel's default: always revalidated) picks up new builds.
# Used by the release workflow; see docs/web.md.
#
#   tools/web/vercel-output.sh [dist dir]   (default: dist)
#       writes .vercel/output/
set -euo pipefail

dist=${1:-dist}
out=.vercel/output
[ -f "$dist/index.html" ] || { echo "no $dist/index.html - run trunk build --release first" >&2; exit 1; }
rm -rf "$out"
mkdir -p "$out"
cp -R "$dist" "$out/static"
cat > "$out/config.json" <<'JSON'
{
  "version": 3,
  "routes": [
    {
      "src": "^/.+-[0-9a-f]{8,}(_bg)?\\.(wasm|js|png)$",
      "headers": { "cache-control": "public, max-age=31536000, immutable" },
      "continue": true
    },
    { "handle": "filesystem" }
  ]
}
JSON
echo "wrote $out from $dist"
