# Dev environment notes

Development so far has happened inside WSL (Linux), even though the shipped
target is a native Windows floating app.

## Rust toolchain

Installed via `rustup` mid-project (wasn't present before). It's wired into
`~/.zshenv`, `~/.profile`, and `~/.bashrc`, so any **new** shell picks up
`cargo`/`rustc` automatically. A shell that was already open before the
install won't have it on `PATH` - fix with:

```
source "$HOME/.cargo/env"
```

or just open a new terminal tab.

## Running/display on WSL

- WSLg forwards the window to the native Windows desktop (`DISPLAY=:0`,
  `WAYLAND_DISPLAY=wayland-0` are set).
- You'll see noisy `libEGL` / `MESA: error: ZINK: failed to choose pdev`
  warnings on startup - these are harmless; the app falls back to a
  software GL renderer and runs fine.
- No screenshot tool (`import`/`scrot`/`gnome-screenshot`) or input injector
  (`xdotool`) is installed and there's no passwordless sudo. Use the app's own
  dev hooks (`src/dev.rs`) instead to capture the real rendered frame:

  ```
  TYM_DATA_DIR=/tmp/tym-data \
  TYM_DEV_START=liu_kang \
  TYM_DEV_TYPE_WORDS=10 \
  TYM_DEV_SCREENSHOT=/path/shot.png \
  cargo run
  ```

  - `TYM_DATA_DIR=<dir>` saves/loads `progress.json` there instead of the OS
    data dir. Always set it for dev runs so they don't touch real progress.

  - `TYM_DEV_START=<sprite prefix>` skips character select and starts a
    match as that character (`liu_kang`, `kano`, `sub_zero`, ...).
  - `TYM_DEV_SELECT=<sprite prefix>` puts the character select cursor on
    that fighter at startup (e.g. to see a locked fighter's unlock hint).
  - `TYM_DEV_TYPE=<text>` feeds text into the typing test at startup.
  - `TYM_DEV_TYPE_WORDS=<n>` correctly types the test's first n words at
    startup, before any `TYM_DEV_TYPE` text (useful for a known WPM; e.g. with `TYM_DEV_DELAY=31.5` to see a
    finished round).
  - `TYM_DEV_SCREENSHOT=<path.png>` saves the full 500x700 window after
    `TYM_DEV_DELAY` seconds (default 1.0) and quits. The cabinet screen is
    the (32,151)-(467,441) crop of it.

## X11 runs: monitor scale changes and real key presses

Running under X11 (XWayland) instead of Wayland lets you change the scale
factor live and inject keys, which WSLg's Wayland doesn't allow.

- winit's X11 backend needs `libxkbcommon-x11`, which isn't installed. Fetch it
  without root into a scratch dir (`<dir>` below):

  ```
  cd <dir> && apt-get download libxkbcommon-x11-0 libxcb-xkb1
  for f in *.deb; do dpkg-deb -x $f .; done
  ln -s libxkbcommon-x11.so.0 usr/lib/x86_64-linux-gnu/libxkbcommon-x11.so
  ```

- Then run with `DISPLAY=:0 WAYLAND_DISPLAY= LD_LIBRARY_PATH=<dir>/usr/lib/x86_64-linux-gnu`.
- `python3 tools/xsettings.py 144 96 2.5` (start it before the app) acts as
  an XSETTINGS manager: Xft/DPI 144 (1.5x), then 96 (1x) after 2.5s. winit
  sees that as a `ScaleFactorChanged`, the same path as dragging the window to
  a monitor with a different scale on Windows. Check the window with
  `xwininfo -name "Test Your Might"` (should be 750x1050 at 1.5x, 500x700 at
  1x) and grab a frame with `TYM_DEV_SCREENSHOT`.
- `python3 tools/xkey.py r Return` presses keys (XTest) in the focused
  window, e.g. to check keys on the results panel after
  `TYM_DEV_TYPE_WORDS=10` and a 30s wait.

## Windows packaging (not done yet)

Still need one of:
- Cross-compiling from WSL to `x86_64-pc-windows-gnu`/`msvc`, or
- Building natively on Windows with `cargo build --release`.

Either way the output is a single `.exe` with no installer, matching the
"no install file" requirement.
