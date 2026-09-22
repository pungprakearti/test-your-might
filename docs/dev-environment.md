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
- No screenshot tool (`import`/`scrot`/`gnome-screenshot`) is installed and
  there's no passwordless sudo, so visual verification during dev has relied
  on the user looking at the actual window rather than automated screenshots.

## Windows packaging (not done yet)

Still need one of:
- Cross-compiling from WSL to `x86_64-pc-windows-gnu`/`msvc`, or
- Building natively on Windows with `cargo build --release`.

Either way the output is a single `.exe` with no installer, matching the
"no install file" requirement.
