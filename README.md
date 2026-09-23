# Test Your Might

A toy app for practicing typing, built for fun. It's a monkeytype-style
typing test rendered inside a Mortal Kombat arcade cabinet, running as a
small, lightweight floating window with no installer required.

Not a serious product, just a hobby project.

## Stack

- **Rust** - compiles to a single native binary, no runtime/installer needed.
- **eframe / egui** (glow backend) - windowing and immediate-mode rendering.
- **image** - decodes the PNG art (cabinet, screens, fighter sprites,
  materials), all embedded in the binary via `include_bytes!`.
- **rand** - picks words for the typing test and the CPU opponent.

The window is a fixed 500x700, undecorated, always-on-top, and draggable by
clicking the cabinet art outside the screen area.

## Running it

Requires Rust (install via [rustup](https://rustup.rs)).

```
cd test-your-might
cargo run
```

This builds and launches the app. On WSL with WSLg, the window appears
directly on the Windows desktop.

For a standalone binary (no `cargo` needed to launch it):

```
cargo build --release
./target/release/test-your-might
```

The release binary is the whole app - just copy and run it, nothing to
install.
