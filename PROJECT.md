# Test Your Might

Monkeytype-style typing test rendered inside a Mortal Kombat arcade cabinet,
built as a lightweight Rust desktop app (no installer, single binary).

Current version: see `Cargo.toml` (`version`). Bump this on every commit.

## Key facts

- Window is fixed at 500x700 to match `mk-cabinet.png`.
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

## Not yet built

- Three-test average + wood/stone/iron/ruby/diamond tier thresholds.
- Character select screen and per-character unlock conditions (including the
  10-non-consecutive-days-played tracker for Scorpion).
- Local persistence of test history (times, wpm, accuracy).
- Sprites (user is providing these separately).
- Windows build/packaging (developed so far on WSL/Linux; need a cross-build
  or native Windows build pass before shipping the floating always-on-top
  window on actual Windows).

## Open questions

- None blocking right now; typing test core is working and approved.
