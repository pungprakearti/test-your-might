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
  `test_your_might.rs`, `typing_test.rs`) with `handle_input`/`draw`.
  `fighter.rs` holds the `Character` enum, sprite loading, and pose frames.
- Fighters: confirming a character starts a match with that character as
  player 1 and a random *different* character as player 2/CPU. Sprites are
  182x224 frames 01-09 drawn at native size with the canvas bottom-center on
  `PLAYER1_FOOT`/`PLAYER2_FOOT` (lines up with the marker pixels baked into
  `tym-bg.png`). Poses: idle = frames 1-4 looping, strike = 5-7 holding on 7,
  victory = 8-9 holding on 9; 0.2s per frame. Only idle is used so far.
- Breakable material (`tym-<material>-N.png`, 182x58, currently always
  `tym-wood-1.png` for both players): drawn at native size, full opacity, in
  front of the fighters. Top-left `PLAYER1_MATERIAL` = (36, 119) and
  `PLAYER2_MATERIAL` = (210, 119), in `tym-bg.png` pixel coords (both
  user-confirmed) - each exactly fills one of the green bracket boxes baked
  into `tym-bg.png` (found via `material-placement.png`).
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
- Test Your Might gameplay: both fighters idle in place behind their wood
  slabs, but there's no strike/victory triggering (pass rules TBD), and
  nothing leads from this screen into the typing test.
- Choosing the material per player/tier (wood/stone/steel/ruby/diamond) and
  using the `-2` (broken) variants; only `tym-wood-1.png` is wired up. The
  `*_placement.png` files are reference art only and aren't drawn.
- Local persistence of test history (times, wpm, accuracy).
- Windows build/packaging (developed so far on WSL/Linux; need a cross-build
  or native Windows build pass before shipping the floating always-on-top
  window on actual Windows).

## Open questions

- None blocking. Typing test core, fighter placement/idle animation, and
  material placement are working and approved.
- `tym-bg.png` has alignment marker pixels baked in (orange corners, green
  slab brackets, purple floor marks) that show in-app. Possibly swap in a
  clean display copy and keep the marked one as reference - not decided.

## More detail

- Stack + how to run: `README.md`
- Dev environment / toolchain quirks (WSL, PATH): `docs/dev-environment.md`
