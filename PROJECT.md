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
  `(32, 151)` to `(467, 441)` (measured from the PNG, see git history of
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
- The Test Your Might screen beyond its backdrop: confirming a character now
  transitions to it (`src/test_your_might.rs`, `tym-bg.png`, with P1/P2 foot
  anchors defined), but no fighters/slab/gameplay are drawn yet, and nothing
  leads from it into the typing test.
- Hooking up the newly-added `assets/<character>_frame0N.png` sprite frames,
  `assets/*_placement.png` reference art, and `assets/tym-*.png` UI pieces -
  all present in `assets/` but not yet referenced from code.
- Local persistence of test history (times, wpm, accuracy).
- Windows build/packaging (developed so far on WSL/Linux; need a cross-build
  or native Windows build pass before shipping the floating always-on-top
  window on actual Windows).

## Open questions

- None blocking right now; typing test core is working and approved.

## More detail

- Stack + how to run: `README.md`
- Dev environment / toolchain quirks (WSL, PATH): `docs/dev-environment.md`
