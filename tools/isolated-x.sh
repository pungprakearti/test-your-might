#!/bin/sh
# isolated-x.sh <command...>: run a command against a private Xvfb display
# (:99), inside a user+mount namespace, so dev runs never show a window on
# the real desktop or take its keyboard focus. Needs the no-root packages in
# ~/.cache/tym-dev-libs - see docs/dev-environment.md.
set -e
D=$HOME/.cache/tym-dev-libs
exec unshare -rm sh -c '
D=$1; shift
# Xvfb runs /usr/bin/xkbcomp (hardcoded) - overlay the downloaded one there.
mount -t overlay overlay -o lowerdir=$D/xkbbin:/usr/bin /usr/bin
# Private X socket dir (the real one belongs to WSLg).
mount -t tmpfs none /tmp/.X11-unix
LD_LIBRARY_PATH=$D/usr/lib/x86_64-linux-gnu $D/usr/bin/Xvfb :99 -screen 0 2560x1440x24 -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
for i in 1 2 3 4 5 6 7 8 9 10; do [ -S /tmp/.X11-unix/X99 ] && break; sleep 0.3; done
DISPLAY=:99 WAYLAND_DISPLAY= "$@"; STATUS=$?
kill $XVFB; exit $STATUS
' sh "$D" "$@"
