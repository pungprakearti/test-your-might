# Test Your Might

Monkeytype-style typing test rendered inside a Mortal Kombat arcade cabinet,
built as a lightweight Rust desktop app (no installer, single binary).

Current version: see `Cargo.toml` (`version`). Bump this on every commit.

## Key facts

- All image assets live under `assets/` (cabinet/character-select art, plus
  unwired character sprite frames and tym-* UI pieces the user has added but
  not yet hooked up).
- Everything is laid out in 500x700 design points to match
  `assets/mk-cabinet.png`; egui's zoom factor maps them onto the window.
  Window size = half the current monitor's height (`HEIGHT_FRACTION`, or
  `--height <frac>`), cabinet-shaped, regardless of OS scale - user's call,
  "try it, not set in stone". `fit_window` in `src/main.rs` resizes once the
  window position and monitor reading have been steady for 0.4s (Wayland
  gives no window position), converts egui's monitor size/position with
  the zoom they were gathered at (egui only rescales screen_rect on a zoom
  change; using the new zoom caused an endless resize loop under
  `cargo run` on WSLg), retries lost resizes, undoes
  maximize, and always zooms to the window's actual size so the cabinet stays
  in proportion. Background: a coworker (3 monitors, mixed scaling) saw the
  old fixed-points window "blow up"; the closest Windows repro was the old
  build at 175% scale being 875x1225, taller than a 1080p monitor.
  Windows/X11 test tooling: `docs/dev-environment.md`.
- Link icons (user-requested; `LINK_ICONS` in `src/main.rs`): a row on the
  lower panel's bottom right - hockey puck (opens
  https://www.biscuitsinthebasket.com) then GitHub logo (opens the repo,
  `update::REPO_URL`). Each is centered at its own aspect ratio in a 50x50
  box (puck 50x38.9, logo 50x48.4); boxes at x 333 and 403, top y 625,
  70px apart center to center like the coin plate's two slots, the row's
  right edge 30px in from the panel edge mirroring the plate's left inset.
  Gray 75, 125 on hover (user's picks) with a pointing-hand cursor; only
  the drawing is clickable, and those areas are excluded from window
  dragging. PNGs: each SVG rendered as a white silhouette at 200px + 8px
  transparent margin (`cargo run --example render_svg -- <svg> <png> 216
  8`, which fits the drawing, not the viewBox, and prints its aspect;
  without the margin, edges got clipped), mipmapped, tinted when drawn.
- The typing test only renders inside the cabinet's "screen" rect:
  `(32, 151)` to `(467, 441)` inclusive = 436x291, same size as the screen
  art so it draws 1:1 (measured from the PNG, see git history of
  `src/main.rs` for the flood-fill measurement approach).
- GUI stack: `eframe`/`egui` (glow backend). Window is undecorated,
  always-on-top, draggable by clicking the cabinet art outside the screen.
- Mute and Close buttons (user-requested, "larger and prominent"): 28px
  round buttons at the top right over the marquee corner (`MUTE_CENTER`,
  `CLOSE_CENTER` in `src/main.rs`), dark disc + white ring + white icon,
  soft drop shadow (`SHADOW_*`), hover highlight (Close turns red) and a
  tooltip. Mute silences all sound instantly (music keeps its place) and is
  saved in eframe's `app.ron` (`MUTED_KEY`).
- All sound is also silent while the window doesn't have focus
  (user-requested; egui's `viewport().focused`, unknown = focused). Separate
  from the Mute setting: not saved, doesn't change the icon. E2E: on the
  private display, focus changed with `XSetInputFocus` (no WM there, and
  X's default focus-follows-pointer doesn't count as focus for winit).
- Typing test mechanics follow monkeytype's model: fixed word stream,
  per-character correctness tracking, WPM = (correct chars / 5) / minutes,
  accuracy = correct keystrokes / total keystrokes. Timer starts on first
  keypress; WPM/accuracy freeze once the 30s test ends.
- Only 3 lines of words are visible at once; the view scrolls up a line as
  you finish one, monkeytype-style. Backspacing across a full word steps
  back into the previous one (repeatable).
- Typed letters ease/fade into place (`animate_value_with_time`) rather than
  snapping in.
- Code layout: `src/main.rs` holds the window/cabinet shell and screen
  routing; each screen lives in its own module (`char_select.rs`,
  `test_your_might.rs`) with `handle_input`/`draw`. `typing_test.rs` is the
  typing test, drawn by the Test Your Might screen; `fighter.rs` holds the
  `Character` enum, sprite loading, and pose frames; `audio.rs` plays the
  sounds; `dev.rs` has env-var dev
  hooks (skip to a match, inject typing, save a screenshot - see
  `docs/dev-environment.md`).
- Fighters: confirming a character starts a match with that character as
  player 1 and a random *different* character as player 2/CPU. Sprites are
  182x224 frames 01-09 drawn at native size with the canvas bottom-center on
  `PLAYER1_FOOT`/`PLAYER2_FOOT` (lined up with marker pixels that were in
  `tym-bg.png`). Poses: idle = frames 1-4 looping, strike = 5-7 holding on 7,
  victory = 8-9 holding on 9; 0.2s per frame.
- Breakable material (`tym-<material>-N.png`, 182x58, currently always
  `tym-wood-1.png` for both players): drawn at native size, full opacity, in
  front of the fighters. Top-left `PLAYER1_MATERIAL` = (36, 119) and
  `PLAYER2_MATERIAL` = (210, 119), in `tym-bg.png` pixel coords (both
  user-confirmed) - each exactly fills one of the green bracket boxes that
  were marked in `tym-bg.png` (found via `material-placement.png`).
- Gauges (`guage.png`, 33x170), one on the outer side of each fighter:
  top-left `PLAYER1_GAUGE` = (39, 2) and `PLAYER2_GAUGE` = (364, 2) in
  `tym-bg.png` coords, each exactly filling a box of orange-red corner
  markers that were in `tym-bg.png` (user-confirmed). Native size, full
  opacity. See "Gauges and progression" below.
- Test Your Might layer order, back to front: `tym-bg.png`, yellow gauge
  fills, gauges + red target bars, fighters, materials, typing test.
- Gauges and progression (`src/progress.rs`, `src/test_your_might.rs`):
  - Materials go wood -> stone -> steel -> ruby -> diamond; beating a
    material's target moves to the next (diamond is last, and beating it
    keeps you on diamond). Failing any material sends you back to wood; the
    goal still comes from the recent average. Starts on wood at 5 WPM.
  - Target WPM = average of the last 5 runs minus a per-material reduction
    (wood 25%, stone 20%, steel 15%, ruby 10%, diamond 5%), rounded, never
    below 5. Pass = final WPM (rounded) >= target.
  - Red bar height per material (fraction of the 160px gauge window): wood
    35%, stone 50%, steel 65%, ruby 75%, diamond 85%. Yellow fill =
    (speed / target) x bar height, clamped to the gauge, so easier materials
    surge past the bar.
  - Player 1 "speed" blends the rolling last-3s WPM toward the overall test
    WPM as the 30s go on, landing exactly on the final WPM; drawn eased.
  - CPU (player 2) is scripted (`CpuRun`): it charges up, swings over and
    under the bar all round, dips under it at 27s, and climbs back just over
    by the buzzer, so it always looks like it nearly lost. Swings widen and
    the winning margin shrinks with harder materials (wood +/-15%, finishing
    10-18% over; ruby +/-26%, finishing 3-6% over) and it always wins
    below diamond. On diamond (+/-30%) its result is the opposite of the
    player's: it settles exactly on the bar at the buzzer, then
    `resolve_diamond` tips it 3-6% under (player won) or over (player lost)
    and the eased gauge shows that as the strike lands.
  - Floor text: before the first key the header says "type to start   ESC
    choose fighter"; while typing it shows timer/wpm and material + goal.
    When time's up the words are replaced by a results panel
    (`typing_test::draw_panel`, same layout): WPM/ACC vs goal, "<MATERIAL>
    BROKEN!" or "TOO SLOW - <MATERIAL> HELD", the next material + goal, and
    "ENTER next round   ESC choose fighter".
  - Keys: Enter after a round starts the next one (fighters back to
    idle); Esc anywhere on this screen returns to character select, which
    keeps the last pick. A round quit midway isn't recorded.
  - End of round (timer hits 0): both fighters strike (frames 5-7). At impact
    (frame 7, 0.4s in) each fighter who beat the goal gets the broken slab
    (`tym-<material>-2.png`), then after a 0.6s beat (`VICTORY_DELAY_SECS`)
    plays victory (8-9, holding on 9). A fighter who fell short holds on 7
    with the slab intact. The CPU follows the same rules (it only fails on
    diamond, when the player wins).
  - Results panel "next" line: "Next: <MATERIAL>, goal N" after a win,
    "Back to WOOD, goal N" after failing above wood, "Next: WOOD again"
    after failing wood.
  - Saved to `progress.json` (material + every run's wpm/accuracy/target/
    pass/time) in the OS data dir (`directories` crate), or `$TYM_DATA_DIR`.
    Written atomically at the end of each round; an unparseable file is
    moved aside (`progress.json.unreadable-<unix time>`), never overwritten.
  - `--version` (`-v`, `--v`, `-V`) prints the version and exits before
    any other flag runs. Windows release builds call `AttachConsole` on
    startup so flag output reaches the launching terminal (verified in a
    real cmd window).
  - `--height <frac>` flag: window height as a fraction of the monitor's
    (0.1-1.0, default 0.5).
  - `--reset` command-line flag (parsed in `main`) permanently deletes
    `progress.json` (not the `.unreadable-*` backups) before the app starts.
    Unknown flags are ignored with a warning.
- Caret: before a word's first letter is typed it sits 2px left of the
  word (`CARET_WORD_START_SHIFT`, user-requested so it doesn't cover the
  letter; 5px was too far); mid-word it stays on the letter boundary (letters touch there, so
  a shift would cut through the typed letter).
- Typing test lives on the Test Your Might screen, on the concrete floor strip
  of `tym-bg.png`: (0,208)-(436,291), 436x83, under a 1px highlight line at
  row 207. Compact layout (12px header, 3 lines of 14px words), centered
  vertically, over a black wash at alpha 150 so gray untyped words stay
  readable on gray concrete. Letters: correct = teal, wrong = red, rest of
  the current word = light gray (200), upcoming words = gray (150). Letters
  typed past a word's end are shown after it in dark red (max 10 per word,
  `MAX_EXTRA_CHARS`), and the caret follows them. A new test starts with each match. There's no
  separate typing screen anymore.
- App now opens on a character select screen (`cs-background.png` drawn over
  the cabinet screen rect) instead of straight into the typing test. A
  green-frame selector (`cs-selected-1.png`/`cs-selected-2.png`) blinks every
  250ms, starts on the first unlocked fighter, and is moved with arrow keys
  across the 7-portrait cross-shaped grid
  (`CHAR_CELLS` in `src/char_select.rs`); Enter only confirms on an unlocked
  character. A hint line on a dark pill under the grid (centered at
  (218, 264), in the plain stone below the frame edge at y=236) names the
  highlighted fighter and says "ENTER to fight" or how to unlock them.
- Sound (`src/audio.rs`, rodio, MP3s in `assets/sounds/` embedded; sources
  in `assets/sounds/SOURCES.md`), all user-specified: Insert Coin at startup
  (not with `TYM_DEV_START`); UI sound 1 when the character select cursor
  actually moves; Enter on an unlocked fighter plays Music Cue 1 and holds
  on character select for 1s (`CONFIRM_HOLD_SECS`) before the match; "Test
  Your Might" at every round start; Hitsound 31 once at strike impact if
  either slab breaks; when player 1's victory pose starts, "<fighter> wins",
  "Excellent", Claps 2 back to back with a 0.25s gap (`ANNOUNCER_GAP`, my
  pick - the clips have no silence padding). Announcer lines are one
  sequence at a time: a new one (next round) or Esc cuts the old one off.
  - Music: the character select theme loops from startup (not with
    `TYM_DEV_START`) and on returning to character select; Enter on a
    fighter stops it. Every round start plays the Test Your Might theme
    once, alongside the line.
  - My calls, easy to change: music at half volume (`MUSIC_VOLUME`; the
    themes are ~2 LU louder than the announcer and 11 LU louder than the
    click); the character select theme rip fades out, so it loops a 4.087s
    phrase found by autocorrelation (r=0.98 seam, 10ms crossfade).
  - Latency (user reported late cursor clicks): clips are pre-decoded on a
    background thread, effects skip the MP3's near-silent lead-in (16ms on
    the click), and the output buffer is 1024 frames (rodio default ~2048).
    Measured under WSL: ~55-60ms sooner. WSLg adds RDP audio lag on top
    that the app can't affect; untested on real Windows.
  - No output device = silent, no error. E2E-verified by recording the
    real output (see `docs/dev-environment.md`).
- Unlocks (`Character::unlock` in `fighter.rs`, evaluated by
  `Progress::is_unlocked` from saved runs - nothing extra is stored): Liu Kang
  from the start; breaking a material for the first time unlocks wood =
  Johnny Cage, stone = Kano, steel = Raiden, ruby = Sonya Blade, diamond =
  Sub-Zero; Scorpion after finishing a round on 10 different local calendar
  days (any 10, not a streak; `chrono` for local dates). This replaced the
  original "% of first-3-round average" tier idea (user's call). Unlocked
  fighters get their `cs-<name>.png` portrait drawn over the cell; the
  results panel announces new unlocks ("... JOHNNY CAGE UNLOCKED!").

## Not yet built

- The `*_placement.png` files are reference art only and aren't drawn.
- Self-update (`src/update.rs`, prompt in `src/update_prompt.rs`, `--update`
  flag): release builds check GitHub's latest release at startup (via the
  github.com `releases/latest` redirect and `releases/download/<tag>/` URLs,
  not the rate-limited REST API) and offer
  it on character select; installs only minisign-signed assets whose signed
  trusted comment is "test-your-might <asset> <version>" (key:
  `keys/release-signing.pub`; secret in `~/.config/test-your-might/` and the
  `MINISIGN_SECRET_KEY` GitHub secret - see `docs/releasing.md`). Windows
  replaces the exe (self_replace), macOS the whole .app. HTTPS: OS TLS on
  Windows/macOS (company proxy certs work), rustls on Linux (dev only).
  Relaunch keeps only `--height`. E2E: `tools/update-e2e.sh` (CI on
  Windows+macOS). First release with it: v0.0.22 (published and signature-verified;
  0.0.19-0.0.21 were never released).
- Releases: pushing a `v*` tag runs `.github/workflows/release.yml`:
  Windows `.exe` (windows-latest) and a universal (arm64+x86_64) macOS
  `.app`, ad-hoc signed, zipped as `test-your-might-macos.zip`
  (macos-latest); CI (build, test, update E2E) also runs on every push to
  main and on PRs. The publish job (tags only) checks the tag matches
  Cargo.toml, signs with `tools/sign-release.sh`, and uploads `.minisig`s. Release builds use
  `windows_subsystem = "windows"` (no console). Not code-signed/notarized
  (needs paid certs), so SmartScreen/Gatekeeper warn on first launch.
  2026-09-24: on the user's work Windows 11 laptop, antivirus silently
  quarantined the v0.0.22 exe ("Windows cannot access the specified
  device, path, or file"); a re-download ran fine. Authenticode signing
  (e.g. Microsoft Trusted Signing, ~$10/mo) is the proper fix if it recurs.
- Nobody has run the macOS build on a real Mac yet (CI only builds it).
  App icon not set yet on either platform.

## Open questions

- None blocking. Typing test core, fighter placement/idle animation,
  material and gauge placement are working and approved; the gauge fill /
  progression feel is new and awaiting the user's play-test.
- The alignment markers have been removed from `tym-bg.png`; the marked
  version (the source of every position above) is in git at v0.0.9.

## More detail

- Stack + how to run: `README.md`
- Dev environment / toolchain quirks (WSL, PATH): `docs/dev-environment.md`
