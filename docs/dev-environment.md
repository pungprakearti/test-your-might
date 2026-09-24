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
  - `TYM_DEV_SCREENSHOT=<path.png>` saves the whole window after
    `TYM_DEV_DELAY` seconds (default 1.0) and quits. The window is sized
    from the monitor (see `fit_window`), so the image is usually not
    500x700; the cabinet screen is the (32,151)-(467,441) region of the
    500x700 design, scaled to the window.

## Building on Linux: ALSA headers

Sound (rodio/cpal) links against ALSA. The user installed the system
packages (2026-09-24), so a plain `cargo run` works:
`sudo apt install pkg-config libasound2-dev libasound2-plugins`, plus
`~/.asoundrc` with `pcm.!default pulse` / `ctl.!default pulse` so sound
reaches WSLg's PulseAudio (the Windows speakers).

Without root (there's no passwordless sudo for the agent), the same
packages, plus the X11 bits below, are unpacked in `~/.cache/tym-dev-libs`:

```
cd ~/.cache/tym-dev-libs
apt-get download libasound2-dev pkgconf pkgconf-bin libpkgconf3 libasound2-plugins
for f in *.deb; do dpkg-deb -x $f .; done
ln -sf /usr/lib/x86_64-linux-gnu/libasound.so.2 usr/lib/x86_64-linux-gnu/libasound.so
```

Then build with:

```
D=~/.cache/tym-dev-libs
export PKG_CONFIG=$D/usr/bin/pkg-config PKG_CONFIG_SYSROOT_DIR=$D \
  PKG_CONFIG_PATH=$D/usr/lib/x86_64-linux-gnu/pkgconfig \
  LD_LIBRARY_PATH=$D/usr/lib/x86_64-linux-gnu
```

## Hearing and recording sound under WSL

WSL's ALSA has no output device, so by default the app runs silently
("no audio output, playing without sound"). To route it to WSLg's
PulseAudio server (plays on the Windows speakers), write
`~/.cache/tym-dev-libs/asoundrc`:

```
pcm_type.pulse { lib "<D>/usr/lib/x86_64-linux-gnu/alsa-lib/libasound_module_pcm_pulse.so" }
ctl_type.pulse { lib "<D>/usr/lib/x86_64-linux-gnu/alsa-lib/libasound_module_ctl_pulse.so" }
pcm.!default { type pulse }
ctl.!default { type pulse }
```

and run with `ALSA_CONFIG_PATH=/usr/share/alsa/alsa.conf:<D>/asoundrc`.
Record what the app plays, in real time, with
`ffmpeg -f pulse -i RDPSink.monitor -ac 1 -ar 44100 rec.wav`. To check which
sounds played and when, cross-correlate each clip (decoded to 44.1 kHz mono
WAV) against the recording; matches score r ~1.0 even when sounds overlap.
Injected keys (`tools/xkey.py`) go to whichever X window has focus, so on
WSLg's display they can miss the game if the user is typing elsewhere (and
theirs can land in it) - use the private display below where possible. The
character select theme under the cursor click lowers its match to r ~0.4,
so detect with a lower threshold there.

## Private display: dev runs that don't touch the desktop

`TYM_DEV_*` runs and key injection on WSLg's display pop the window up on
the user's Windows desktop, and it takes keyboard focus: if they're typing
elsewhere, their keys land in the game (and injected keys can miss it).
`tools/isolated-x.sh <command...>` runs the command against a private Xvfb
display (:99, 2560x1440) inside a user+mount namespace instead - nothing
shows on the desktop, and `tools/xkey.py` always reaches the game. It needs
Xvfb and friends unpacked (no root) into `~/.cache/tym-dev-libs`:

```
cd ~/.cache/tym-dev-libs
apt-get download xvfb libxfont2 libfontenc1 xserver-common x11-xkb-utils
for f in *.deb; do dpkg-deb -x $f .; done
mkdir -p xkbbin && cp usr/bin/xkbcomp xkbbin/
```

(Xvfb runs `/usr/bin/xkbcomp` by absolute path, so the script overlays
`xkbbin/` onto `/usr/bin` inside the namespace, and mounts a private
`/tmp/.X11-unix`.) Inside, set `LD_LIBRARY_PATH` to
`~/.cache/tym-dev-libs/usr/lib/x86_64-linux-gnu` (libxkbcommon-x11, below)
and, for a silent run, `ALSA_CONFIG_PATH=/nonexistent`. Example:

```
tools/isolated-x.sh env LD_LIBRARY_PATH=$HOME/.cache/tym-dev-libs/usr/lib/x86_64-linux-gnu \
  ALSA_CONFIG_PATH=/nonexistent TYM_DATA_DIR=/tmp/tym-data TYM_DEV_START=liu_kang \
  TYM_DEV_SCREENSHOT=/tmp/shot.png ./target/debug/test-your-might
```

## X11 runs: monitor scale changes and real key presses

Running under X11 (XWayland) instead of Wayland lets you change the scale
factor live and inject keys, which WSLg's Wayland doesn't allow.

- winit's X11 backend needs `libxkbcommon-x11`, which isn't installed. Fetch it
  without root into a scratch dir (`<dir>` below; already unpacked in
  `~/.cache/tym-dev-libs`):

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
  `xwininfo -name "Test Your Might"` (should stay half the monitor's height
  at any scale) and grab a frame with `TYM_DEV_SCREENSHOT`.
- `python3 tools/xkey.py r Return` presses keys (XTest) in the focused
  window, e.g. to check keys on the results panel after
  `TYM_DEV_TYPE_WORDS=10` and a 30s wait.

## Self-update testing

`tools/update-e2e.sh` (Linux, macOS, Git Bash on Windows; CI runs it on
Windows and macOS) builds this checkout and a 99.0.0 copy, signs the copy
with a throwaway key, serves it from a local fake GitHub
(`tools/fake-release-server.py`), and runs the
installed game's `--update`: the good release must install, and tampered,
wrongly-signed, replayed (signed for another version), unsigned, and
not-newer ones must be refused or ignored with the install untouched.

Hooks for trying the in-game prompt against your own fake server (see
`src/update.rs`):

- `TYM_UPDATE_URL=<url>`: use this instead of the GitHub repo URL. The game
  requests `<url>/releases/latest` (expects a redirect to
  `.../releases/tag/<tag>`) and `<url>/releases/download/<tag>/<file>`, the
  same github.com web URLs it uses normally (not the REST API, which is
  limited to 60 unauthenticated requests an hour per IP). Also turns on the
  startup check in debug builds, which otherwise skip it.
  `python3 tools/fake-release-server.py <root> <port> [--throttle]` serves
  `<root>/<repo>/latest` (a tag, e.g. `v99.0.0`) and
  `<root>/<repo>/<tag>/<files>` that way; `--throttle` slows downloads so
  the progress bar is visible. Then use
  `TYM_UPDATE_URL=http://127.0.0.1:<port>/<repo>`.
- `TYM_UPDATE_PUBKEY=<base64>`: trust this key instead of
  `keys/release-signing.pub`.
- `TYM_UPDATE_ASSET=<name>`: the asset to look for (Linux builds aren't
  released, so set it there, e.g. `test-your-might-linux`).

`cargo run --example update_sign -- keygen <dir>` makes a throwaway key pair
(`test.key`, `test.pub`); `... -- sign <key> <file> "test-your-might <asset>
<version>"` writes `<file>.minisig`.

## Windows builds and real-Windows testing from WSL

Official releases are built by GitHub Actions (see README). For local
testing, cross-compile from WSL with `cargo-xwin` (no C code in the project,
so rust-lld is the only linker needed; HTTPS uses the OS's TLS on Windows, so
no C compiler is needed either):

```
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin --locked
XWIN_ACCEPT_LICENSE=1 CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld \
  cargo xwin build --release --target x86_64-pc-windows-msvc
```

Copy the `.exe` somewhere under `/mnt/c/...` and start it with
`powershell.exe -Command "Start-Process 'C:\...\app.exe'"` (set
`$env:TYM_DATA_DIR` first to keep real progress safe). Don't redirect its
stderr from `Start-Process`: PowerShell then waits for the app to exit.

- `tools/windows/drive.ps1` finds the window by process name, reports its
  rect/DPI/maximized state, screenshots it, double-clicks or drags it with
  the real mouse, presses a key (`-Action key -Key Enter`), sends a
  maximize, or closes it.
- Windows can reach a server listening on `127.0.0.1` inside WSL, so a fake
  update server can run in WSL. To see a GUI build's stdout, run it with
  `Start-Process ... -Wait -RedirectStandardOutput <file>`.
- `tools/windows/scale.ps1 -Device '\\.\DISPLAY2' [-Set 150]` reads or sets
  one monitor's display scaling (the undocumented DisplayConfig call the
  Settings app uses). Put it back afterwards.
- eframe keeps its own window state (position, size, egui zoom) in
  `%APPDATA%\Test Your Might\data\app.ron`, separate from `TYM_DATA_DIR`.
  Back it up before testing and restore it after; builds share it, so one
  build's saved zoom leaks into the next launch of another.
- The app is always-on-top: when the user is using one monitor, keep test
  windows (and the saved position in app.ron) on the other.
