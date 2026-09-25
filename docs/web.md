# The web version

The game also runs in a browser, so people can try it without downloading
anything. It's the same crate compiled to WebAssembly (`wasm32-unknown-unknown`)
by [Trunk](https://trunkrs.dev), drawn by eframe on a `<canvas>` in
[`index.html`](../index.html), and hosted on Vercel as plain static files
at https://test-your-might.vercel.app (live since v0.0.25).

## Running it locally

One-time setup:

```
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
```

Then, from the repo root:

```
trunk serve --release
```

and open http://127.0.0.1:8080. Trunk rebuilds and reloads the page when a
source file changes. Under WSL, a Windows browser reaches it at the same
address. `--release` matters here: a debug build works, but its `.wasm` is
~84 MB (vs 4.8 MB) and unoptimized, so it loads slowly and decodes the
images and sounds slowly. Use it only when you need debug assertions or
faster rebuilds.

`trunk build --release` just writes the site to `dist/`: `index.html`, one
JS loader, one `.wasm` (~4.8 MB, ~3.3 MB compressed; nearly all of it is
the embedded PNG and MP3 assets), and the favicon.

## How the code splits

Everything is shared except a few spots that talk to the OS, which pick an
implementation with `#[cfg(target_arch = "wasm32")]`:

| | Desktop | Web |
|---|---|---|
| Start-up | `main` parses flags, opens the window | `main` starts `eframe::WebRunner` on `<canvas id="game">` |
| Sizing | `fit_window` sizes the window from the monitor | `fit_page` zooms the cabinet to the page, standing on its bottom edge |
| Close button, window dragging, `--flags` | yes | no (a page can't close its tab); Mute moves into Close's corner |
| Self-update (`update.rs`, `update_prompt.rs`) | yes | not compiled in: the page is always the latest deploy |
| Sound | `audio/native.rs` (rodio) | `audio/web.rs` (Web Audio) |
| Saved progress | `progress.json` file | `localStorage["test-your-might.progress"]`, same JSON |
| Mute setting | eframe's `app.ron` | eframe's `localStorage` entry |
| Logging (`warn!`) | stderr | browser console |
| Dev hooks (`dev.rs`) | env vars | URL query, e.g. `?TYM_DEV_START=liu_kang` |

`web-time` stands in for `std::time::Instant`/`SystemTime` (which panic on
wasm32; on the desktop it's just `std::time`), `getrandom`'s `js` feature
feeds `rand`, and `chrono`'s `wasmbind` gives local dates for the
days-played unlock. Web-only dependencies are in Cargo.toml's
`cfg(target_arch = "wasm32")` section, so desktop builds are unchanged
(checked: the Linux release binary grew by 1.7 KB, 0.02%, with an
identical dependency tree).

### Sound on the web

MP3s are decoded by rodio exactly as on the desktop, so the character select
theme's loop point and the effects' trimmed lead-in are identical, then
copied into Web Audio `AudioBuffer`s. Decoding happens in the background one
clip per turn of the browser's event loop. Browsers only allow sound after
the player interacts with the page, so the game's `AudioContext` starts out
suspended: Insert Coin and the character select theme start on the first
key press or click. Every key press, click, or tap resumes the context in
case the browser suspended it again.

### Build quirk

The release profile's `strip = true` also strips the wasm section that tells
`wasm-opt` which WebAssembly features the module uses, so `index.html` lists
Rust's default feature set in `data-wasm-opt-params`. `Trunk.toml` pins a
recent `wasm-opt` that knows all of them.

## Deploying (Vercel)

CI does the deploying (`.github/workflows/release.yml`):

- job `web` lints for wasm32, runs `trunk build --release`, and packages
  `dist/` with `tools/web/vercel-output.sh` into Vercel's Build Output API
  layout (`.vercel/output/`), so Vercel serves the files as they are and
  doesn't try to build anything (it has no Rust). Hashed file names are
  cached forever; `index.html` is always revalidated.
- job `deploy-web` runs after every other job passes and runs
  `vercel deploy --prebuilt`: a **preview** deployment for pull requests
  (the URL is in the run's summary) and **production** for `v*` tags, so
  the site updates together with the desktop release.

Current setup (2026-09-25): Vercel project `test-your-might`
(`prj_n2DsLjKBzoya3PXFhoEN1N4GDRCo`) in the team "pungprakearti's projects"
(`team_q0Euaub96LYgIpWP0FmGJySf`), with no Git connection. CI deploys with
the token "github-actions test-your-might", scoped to that project only and
set to never expire; revoke it in Vercel's account settings > Tokens if it
leaks, and put a new one in the `VERCEL_TOKEN` secret. Preview deployments
are behind Vercel's login (Deployment Protection); production is public.

One-time setup, for a new project:

1. Create a Vercel project for the repo (Framework Preset: Other). Don't
   connect Vercel's own Git integration, or it will try to build on every
   push too. If it's already connected, turn off its automatic
   deployments (for example `"git": {"deploymentEnabled": false}`).
2. Create a token at https://vercel.com/account/tokens.
3. Get the org and project IDs: run `npx vercel link` in the repo, then read
   `.vercel/project.json` (`orgId`, `projectId`). `.vercel/` is gitignored.
4. Add the repository secrets `VERCEL_TOKEN`, `VERCEL_ORG_ID`, and
   `VERCEL_PROJECT_ID` (Settings > Secrets and variables > Actions).

Without the secrets, `deploy-web` logs a warning and skips the deploy.

To deploy by hand: `trunk build --release && tools/web/vercel-output.sh &&
npx vercel deploy --prebuilt` (add `--prod` for production).

## Known limits

- It needs a physical keyboard. On phones and tablets the page shows, but
  there's no way to type.
- Progress lives in each browser separately (and private windows forget it).
