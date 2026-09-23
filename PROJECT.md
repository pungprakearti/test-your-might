# Test Your Might

Monkeytype-style typing test rendered inside a Mortal Kombat arcade cabinet,
built as a lightweight Rust desktop app (no installer, single binary).

Current version: see `Cargo.toml` (`version`). Bump this on every commit.

## Key facts

- All image assets live under `assets/` (cabinet/character-select art, plus
  unwired character sprite frames and tym-* UI pieces the user has added but
  not yet hooked up).
- Window is fixed at 500x700 to match `assets/mk-cabinet.png`.
- The typing test only renders inside the cabinet's "screen" rect:
  `(32, 151)` to `(467, 441)` inclusive = 436x291, same size as the screen
  art so it draws 1:1 (measured from the PNG, see git history of
  `src/main.rs` for the flood-fill measurement approach).
- GUI stack: `eframe`/`egui` (glow backend). Window is undecorated,
  always-on-top, draggable by clicking the cabinet art outside the screen,
  closable via the X button top-right.
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
  `Character` enum, sprite loading, and pose frames; `dev.rs` has env-var dev
  hooks (skip to a match, inject typing, save a screenshot - see
  `docs/dev-environment.md`).
- Fighters: confirming a character starts a match with that character as
  player 1 and a random *different* character as player 2/CPU. Sprites are
  182x224 frames 01-09 drawn at native size with the canvas bottom-center on
  `PLAYER1_FOOT`/`PLAYER2_FOOT` (lined up with marker pixels that were in
  `tym-bg.png`). Poses: idle = frames 1-4 looping, strike = 5-7 holding on 7,
  victory = 8-9 holding on 9; 0.2s per frame. Only idle is used so far.
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
    material's target moves to the next (diamond is last). Starts on wood at
    5 WPM.
  - Target WPM = average of the last 5 runs minus a per-material reduction
    (wood 25%, stone 20%, steel 15%, ruby 10%, diamond 5%), rounded, never
    below 5. Pass = final WPM (rounded) >= target.
  - Red bar height per material (fraction of the 160px gauge window): wood
    35%, stone 50%, steel 65%, ruby 75%, diamond 85%. Yellow fill =
    (speed / target) x bar height, clamped to the gauge, so easier materials
    surge past the bar.
  - Player 1 "speed" blends the rolling last-3s WPM toward the overall test
    WPM as the 30s go on, landing exactly on the final WPM; drawn eased.
  - CPU (player 2) is scripted: it always ends above the bar, except on
    diamond where it never reaches it.
  - Header shows material + goal while typing, then "<MATERIAL> BROKEN!" or
    "FAILED"; R starts the next round.
  - Saved to `progress.json` (material + every run's wpm/accuracy/target/
    pass/time) in the OS data dir (`directories` crate), or `$TYM_DATA_DIR`.
- Typing test lives on the Test Your Might screen, on the concrete floor strip
  of `tym-bg.png`: (0,208)-(436,291), 436x83, under a 1px highlight line at
  row 207. Compact layout (12px header, 3 lines of 14px words), centered
  vertically, over a black wash at alpha 150 so gray untyped words stay
  readable on gray concrete. A new test starts with each match. There's no
  separate typing screen anymore.
- App now opens on a character select screen (`cs-background.png` drawn over
  the cabinet screen rect) instead of straight into the typing test. A
  green-frame selector (`cs-selected-1.png`/`cs-selected-2.png`) blinks every
  250ms and is moved with arrow keys across the 7-portrait cross-shaped grid
  (`CHAR_CELLS` in `src/char_select.rs`); Enter only confirms on an unlocked
  character (currently just Liu Kang).
- Each `CHAR_CELLS` entry carries an `UnlockCondition` flag (`Default`,
  `WpmTier(Wood/Stone/Iron/Ruby/Diamond)`, or `PlayedDays(10)`) mapped
  alphabetically: Wood=Johnny Cage, Stone=Kano, Iron=Raiden, Ruby=Sonya Blade,
  Diamond=Sub-Zero, PlayedDays(10)=Scorpion, Default=Liu Kang. `is_unlocked`
  is still a placeholder that only returns true for `Default`, since the tier
  averaging and day-tracking to evaluate the others for real isn't built.
  When a character *is* unlocked, its `cs-<name>.png` portrait is drawn over
  its cell in addition to becoming selectable.

## Not yet built

- Three-test average + wood/stone/iron/ruby/diamond tier thresholds, and
  wiring `is_unlocked` up to them plus a played-days tracker for Scorpion.
- Strike/victory animations and broken-material (`-2`) sprites when a round
  ends; fighters just idle for now.
- The `*_placement.png` files are reference art only and aren't drawn.
- Character-unlock tiers (`WpmTier` in `char_select.rs`, still named
  Wood/Stone/Iron/...) aren't connected to the material progression yet.
- Windows build/packaging (developed so far on WSL/Linux; need a cross-build
  or native Windows build pass before shipping the floating always-on-top
  window on actual Windows).

## Open questions

- None blocking. Typing test core, fighter placement/idle animation,
  material and gauge placement are working and approved; the gauge fill /
  progression feel is new and awaiting the user's play-test.
- The alignment markers have been removed from `tym-bg.png`; the marked
  version (the source of every position above) is in git at v0.0.9.

## More detail

- Stack + how to run: `README.md`
- Dev environment / toolchain quirks (WSL, PATH): `docs/dev-environment.md`
