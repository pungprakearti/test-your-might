// Dev-only hooks, driven by environment variables, for checking the real
// rendered output without a screenshot tool or synthetic key presses (see
// docs/dev-environment.md). All are no-ops unless their variable is set.
// Set TYM_DATA_DIR too (see progress.rs) so dev runs don't touch real saved
// progress.
//
//   TYM_DEV_START=<sprite prefix>  skip character select and start a match
//                                   as that character (e.g. liu_kang)
//   TYM_DEV_SELECT=<sprite prefix> put the character select cursor on that
//                                   fighter at startup
//   TYM_DEV_TYPE=<text>            feed this text to the typing test at start
//   TYM_DEV_TYPE_WORDS=<n>         correctly type the test's first n words at
//                                   start
//   TYM_DEV_SCREENSHOT=<path.png>  save a screenshot after TYM_DEV_DELAY
//                                   seconds (default 1.0), then quit
//                                   (desktop only)
//
// On the web, where there's no environment, the same names are read from the
// page's URL query instead, e.g. index.html?TYM_DEV_START=liu_kang.

use eframe::egui;

use crate::fighter::Character;

#[cfg(not(target_arch = "wasm32"))]
fn var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

#[cfg(target_arch = "wasm32")]
fn var(name: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search).ok()?.get(name)
}

pub fn start_character() -> Option<Character> {
    let name = var("TYM_DEV_START")?;
    let found = Character::ALL.into_iter().find(|c| c.sprite_prefix() == name);
    if found.is_none() {
        warn!("TYM_DEV_START: unknown character {name:?}");
    }
    found
}

pub fn select_character() -> Option<Character> {
    let name = var("TYM_DEV_SELECT")?;
    Character::ALL.into_iter().find(|c| c.sprite_prefix() == name)
}

pub fn typed_text() -> Option<String> {
    var("TYM_DEV_TYPE")
}

pub fn typed_words() -> Option<usize> {
    var("TYM_DEV_TYPE_WORDS")?.parse().ok()
}

pub struct Screenshot {
    path: Option<String>,
    delay: f64,
    requested: bool,
}

impl Screenshot {
    pub fn from_env() -> Self {
        let delay = std::env::var("TYM_DEV_DELAY").ok().and_then(|s| s.parse().ok()).unwrap_or(1.0);
        Self { path: std::env::var("TYM_DEV_SCREENSHOT").ok(), delay, requested: false }
    }

    // Call once per frame: requests the screenshot when due, and saves it
    // and closes the app once egui delivers it.
    pub fn update(&mut self, ctx: &egui::Context) {
        let Some(path) = &self.path else {
            return;
        };
        if !self.requested {
            if ctx.input(|i| i.time) >= self.delay {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                self.requested = true;
            }
            ctx.request_repaint();
            return;
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            let [w, h] = image.size;
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            match image::save_buffer(path, &rgba, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
                Ok(()) => eprintln!("TYM_DEV_SCREENSHOT: saved {path}"),
                Err(e) => eprintln!("TYM_DEV_SCREENSHOT: failed to save {path}: {e}"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            ctx.request_repaint();
        }
    }
}
