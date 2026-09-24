#!/usr/bin/env bash
# End-to-end test of self-updating (src/update.rs) against a local fake
# GitHub. Builds this checkout as the "installed" game and a copy of it with
# version 99.0.0 as the "new release", packages the release the way the
# release workflow does, signs it with a throwaway key, serves it over HTTP,
# and runs the installed game's `--update`. Checks that a good release
# installs, and that tampered, wrongly-signed, replayed, unsigned, and
# not-newer releases are refused or ignored without touching the install.
#
# Runs on Linux, macOS, and Windows (Git Bash). Needs cargo and python3.
# The fake GitHub is tools/fake-release-server.py.
# Usage: tools/update-e2e.sh
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
server_pid=
cleanup() {
    [ -n "$server_pid" ] && kill "$server_pid" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT

case "$(uname -s)" in
    Darwin) platform=macos ;;
    MINGW* | MSYS* | CYGWIN*) platform=windows ;;
    *) platform=linux ;;
esac
# The first Python that actually runs (on Windows, python3 can be a
# Microsoft Store placeholder).
python=
for candidate in python3 python; do
    if "$candidate" -c "import sys" >/dev/null 2>&1; then python=$candidate && break; fi
done
[ -n "$python" ] || { echo "FAIL: needs python3" >&2; exit 1; }
exe_suffix=; [ $platform = windows ] && exe_suffix=.exe
case $platform in
    windows) asset=test-your-might.exe ;;
    macos) asset=test-your-might-macos.zip ;;
    # Linux isn't released; the asset name comes from TYM_UPDATE_ASSET.
    linux) asset=test-your-might-linux ;;
esac

# What the install folder should hold, before and after an update.
installed_name="test-your-might$exe_suffix"; [ $platform = macos ] && installed_name="Test Your Might.app"

step() { printf '\n== %s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

step "build the installed game ($(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1))"
(cd "$root" && cargo build --release --quiet)
cp "$root/target/release/test-your-might$exe_suffix" "$work/old$exe_suffix"
old_version=$("$work/old$exe_suffix" --version)
echo "$old_version"

step "build the new release (99.0.0)"
mkdir "$work/src99"
cp -R "$root/Cargo.toml" "$root/Cargo.lock" "$root/src" "$root/assets" "$root/keys" "$work/src99/"
sed -i.bak 's/^version = ".*"/version = "99.0.0"/' "$work/src99/Cargo.toml"
(cd "$work/src99" && CARGO_TARGET_DIR="$root/target/update-e2e" cargo build --release --quiet)
cp "$root/target/update-e2e/release/test-your-might$exe_suffix" "$work/new$exe_suffix"
"$work/new$exe_suffix" --version | grep -q "99.0.0" || fail "new build doesn't report 99.0.0"

step "package and sign"
mkdir "$work/www"
package() { # <binary> <version> <out file>
    if [ $platform = macos ]; then
        rm -rf "$work/pkg" && mkdir "$work/pkg"
        "$root/tools/macos/bundle.sh" "$1" "$2" "$work/pkg" >/dev/null
        ditto -c -k --keepParent "$work/pkg/Test Your Might.app" "$3"
    else
        cp "$1" "$3"
    fi
}
package "$work/new$exe_suffix" 99.0.0 "$work/www/$asset"
sign=("$root/target/release/examples/update_sign$exe_suffix")
(cd "$root" && cargo build --release --quiet --example update_sign)
mkdir "$work/key" "$work/otherkey"
"${sign[@]}" keygen "$work/key"
"${sign[@]}" keygen "$work/otherkey"
"${sign[@]}" sign "$work/key/test.key" "$work/www/$asset" "test-your-might $asset 99.0.0"

# Variants: each is a fake repo for tools/fake-release-server.py, with its
# latest tag in <variant>/latest and that release's files in <variant>/<tag>/.
variant() { # <name> [<tag>]
    local tag=${2:-v99.0.0}
    mkdir -p "$work/www/$1/$tag"
    echo "$tag" > "$work/www/$1/latest"
    cp "$work/www/$asset" "$work/www/$asset.minisig" "$work/www/$1/$tag/"
    echo "$work/www/$1/$tag"
}
variant good >/dev/null
dir=$(variant tampered)
"$python" - "$dir/$asset" <<'PY'
import sys
p = sys.argv[1]
data = bytearray(open(p, "rb").read())
data[len(data) // 2] ^= 0xFF
open(p, "wb").write(bytes(data))
PY
dir=$(variant wrongkey)
"${sign[@]}" sign "$work/otherkey/test.key" "$dir/$asset" "test-your-might $asset 99.0.0"
dir=$(variant replay)
"${sign[@]}" sign "$work/key/test.key" "$dir/$asset" "test-your-might $asset 98.0.0"
dir=$(variant unsigned)
rm "$dir/$asset.minisig"
variant notnewer v0.0.1 >/dev/null

port=$((20000 + RANDOM % 20000))
"$python" "$root/tools/fake-release-server.py" "$work/www" "$port" &
server_pid=$!
for _ in $(seq 50); do
    "$python" -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:$port/good/releases/download/v99.0.0/$asset.minisig')" 2>/dev/null && break
    sleep 0.2
done

# A fresh install of the old game; prints the path of its executable.
install_old() {
    rm -rf "$work/install" && mkdir "$work/install"
    if [ $platform = macos ]; then
        "$root/tools/macos/bundle.sh" "$work/old" "$(echo "$old_version" | awk '{print $NF}')" "$work/install" >/dev/null
        echo "$work/install/Test Your Might.app/Contents/MacOS/test-your-might"
    else
        cp "$work/old$exe_suffix" "$work/install/test-your-might$exe_suffix"
        echo "$work/install/test-your-might$exe_suffix"
    fi
}
# Runs `--update` against variant $1; output in $work/out, status in $status.
run_update() {
    set +e
    TYM_UPDATE_URL="http://127.0.0.1:$port/$1" \
    TYM_UPDATE_PUBKEY="$(cat "$work/key/test.pub")" \
    TYM_UPDATE_ASSET="$asset" \
        "$exe" --update > "$work/out" 2>&1
    status=$?
    set -e
    sed 's/^/   | /' "$work/out"
}
expect_refused() { # <variant> <expected message>
    step "$1: must be refused"
    exe=$(install_old)
    run_update "$1"
    [ $status -ne 0 ] || fail "$1: --update succeeded"
    grep -q "$2" "$work/out" || fail "$1: expected \"$2\""
    [ "$("$exe" --version)" = "$old_version" ] || fail "$1: installed game changed"
    check_no_leftovers "$1"
}
check_no_leftovers() {
    [ "$(ls -A "$work/install")" = "$installed_name" ] || fail "$1: leftover files: $(ls -A "$work/install")"
}

expect_refused tampered "signature check"
expect_refused wrongkey "signature check"
expect_refused replay "signed for a different version"
expect_refused unsigned "no signature"

step "notnewer: must be ignored"
exe=$(install_old)
run_update notnewer
[ $status -eq 0 ] || fail "notnewer: --update failed"
grep -q "up to date" "$work/out" || fail "notnewer: expected \"up to date\""
[ "$("$exe" --version)" = "$old_version" ] || fail "notnewer: installed game changed"

step "good: must install"
exe=$(install_old)
run_update good
[ $status -eq 0 ] || fail "good: --update failed"
grep -q "Updated to 99.0.0" "$work/out" || fail "good: expected \"Updated to 99.0.0\""
sleep 2 # Windows: the old exe is deleted by a helper after we exit
new_reported=$("$exe" --version)
echo "   installed game now reports: $new_reported"
[ "$new_reported" = "Test Your Might 99.0.0" ] || fail "good: installed game reports \"$new_reported\""
check_no_leftovers good
if [ $platform = macos ]; then
    app="$work/install/Test Your Might.app"
    plutil -extract CFBundleShortVersionString raw "$app/Contents/Info.plist" | grep -qx 99.0.0 ||
        fail "good: Info.plist not updated"
    codesign --verify --deep "$app" || fail "good: new app's signature doesn't verify"
fi

step "all update checks passed"
