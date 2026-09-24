// Release builds are a windowed app on Windows: no console window behind the
// game. Debug builds keep the console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Test Your Might - arcade cabinet app scaffold.
// Everything is laid out in 500x700 design points to match mk-cabinet.png;
// the window itself is sized from the monitor's height (see fit_window). Each
// screen (character select, then Test Your Might with the typing test on its
// floor) renders only inside the cabinet's "screen" rect of that image:
// (32,151) -> (467,441) inclusive, 436x291. See char_select.rs,
// test_your_might.rs, and typing_test.rs for the individual screens.

mod char_select;
mod dev;
mod fighter;
mod progress;
mod test_your_might;
mod typing_test;
mod update;
mod update_prompt;

use eframe::egui;
use std::time::Duration;

use char_select::CharSelectScreen;
use test_your_might::TestYourMightScreen;
use update_prompt::UpdatePrompt;

const WINDOW_W: f32 = 500.0;
const WINDOW_H: f32 = 700.0;
const WINDOW_SIZE: egui::Vec2 = egui::vec2(WINDOW_W, WINDOW_H);
// Window height as a fraction of the monitor's height (see fit_window).
const HEIGHT_FRACTION: f32 = 0.5;
// How long the window must sit still on a monitor before it's resized for it.
const SETTLE_SECS: f64 = 0.4;
// How often to re-ask for the right window size while it's still wrong.
const RESIZE_RETRY_SECS: f64 = 1.0;

// Screen rect measured from mk-cabinet.png (flood-filled bounding box of the
// blue-gray panel). The panel's last pixel column/row is 467/441, so the
// exclusive max is one past that - 436x291, matching the screen art exactly so
// it's drawn 1:1 rather than resampled.
const SCREEN_MIN: egui::Pos2 = egui::pos2(32.0, 151.0);
const SCREEN_MAX: egui::Pos2 = egui::pos2(468.0, 442.0);

#[derive(PartialEq)]
enum Screen {
    CharSelect,
    TestYourMight,
}

pub(crate) fn load_texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let pixels = img.into_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
    Some(ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR))
}

struct App {
    bg_texture: Option<egui::TextureHandle>,
    screen: Screen,
    char_select: CharSelectScreen,
    test_your_might: TestYourMightScreen,
    dev_screenshot: dev::Screenshot,
    updater: UpdatePrompt,
    height_fraction: f32,
    fit: WindowFit,
}

// fit_window's state between frames. Sizes are physical pixels.
#[derive(Default)]
struct WindowFit {
    // Size the window should be, from the monitor it last settled on.
    target_px: Option<egui::Vec2>,
    // When we last asked the OS for target_px. A request can be lost (on
    // Windows, winit's own resize for a DPI change lands after ours), so it's
    // retried every RESIZE_RETRY_SECS while the window is still off.
    resize_requested_at: Option<f64>,
    // Zoom factor the current frame's input was gathered with (see
    // fit_window).
    input_zoom: Option<f32>,
    last_outer_px: Option<egui::Pos2>,
    last_wanted_px: Option<egui::Vec2>,
    // Last time the window moved or its monitor-based size changed.
    changed_at: f64,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, height_fraction: f32) -> Self {
        let bg_texture = load_texture(&cc.egui_ctx, "cabinet-bg", include_bytes!("../assets/mk-cabinet.png"));
        let test_your_might = TestYourMightScreen::new(cc);
        let mut app = Self {
            bg_texture,
            screen: Screen::CharSelect,
            char_select: CharSelectScreen::new(cc, test_your_might.progress()),
            test_your_might,
            dev_screenshot: dev::Screenshot::from_env(),
            updater: UpdatePrompt::start(&cc.egui_ctx),
            height_fraction,
            fit: WindowFit::default(),
        };
        // fit_window owns the zoom factor.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        if let Some(character) = dev::select_character() {
            app.char_select.dev_select(character);
        }
        if let Some(character) = dev::start_character() {
            app.test_your_might.start_match(&cc.egui_ctx, character, 0.0);
            app.screen = Screen::TestYourMight;
        }
        // Correct words first, then any free text after them.
        if let Some(n) = dev::typed_words() {
            app.test_your_might.dev_type_words(n);
        }
        if let Some(text) = dev::typed_text() {
            app.test_your_might.dev_type(&text);
        }
        app
    }

    // Sizes the window from the monitor it's on: HEIGHT_FRACTION (or
    // --height) of the monitor's height, cabinet-shaped, whatever the OS
    // scale setting. Everything is laid out in fixed 500x700 design points
    // and the zoom factor maps those onto the window, so the whole cabinet
    // scales together. Zoom always follows the window's actual size, so the
    // design stays in proportion even if the OS resizes the window (DPI
    // change mid-drag, a refused resize, a restored saved size). A monitor
    // change only resizes once the window position and the monitor reading
    // have both been steady for SETTLE_SECS, so it doesn't jump around
    // mid-drag or on a one-off odd reading (and still works on Wayland,
    // where the window position isn't known). Returns where the design sits
    // in the window, in points.
    fn fit_window(&mut self, ctx: &egui::Context) -> egui::Rect {
        let now = ctx.input(|i| i.time);
        let ppp = ctx.pixels_per_point();
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        let fit = &mut self.fit;
        // egui-winit converts monitor_size/outer_rect to points with the zoom
        // in effect when it gathered this frame's input. After a zoom change
        // egui switches to the new zoom but only rescales screen_rect, so
        // converting them back with ctx.pixels_per_point() would be off by
        // the zoom ratio for a frame - enough to set off an endless resize
        // loop. Use the zoom the input was gathered with instead.
        let input_ppp = native * fit.input_zoom.unwrap_or(ctx.zoom_factor());
        let (monitor_px, outer_px, maximized, fullscreen) = ctx.input(|i| {
            let v = i.viewport();
            (v.monitor_size.map(|s| s * input_ppp), v.outer_rect.map(|r| r.min * input_ppp), v.maximized, v.fullscreen)
        });

        // A maximized/fullscreen window can't take our size.
        if maximized == Some(true) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        }
        if fullscreen == Some(true) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }

        // Physical pixels per design point for this monitor.
        let wanted_scale = monitor_px.map_or(native, |m| self.height_fraction * m.y / WINDOW_H);
        let wanted_px = (WINDOW_SIZE * wanted_scale).round();

        if outer_px != fit.last_outer_px || Some(wanted_px) != fit.last_wanted_px {
            fit.last_outer_px = outer_px;
            fit.last_wanted_px = Some(wanted_px);
            fit.changed_at = now;
        }
        // (update's regular repaint keeps checking this.)
        let settled = now - fit.changed_at >= SETTLE_SECS;
        if fit.target_px.is_none() || settled {
            fit.target_px = Some(wanted_px);
        }
        let target_px = fit.target_px.unwrap_or(wanted_px);

        let window_px = ctx.screen_rect().size() * ppp;
        let off = (window_px - target_px).abs().max_elem() > 1.5;
        let zoom = (window_px.x / WINDOW_W).min(window_px.y / WINDOW_H) / native;
        if !off {
            fit.resize_requested_at = None;
        } else if settled && fit.resize_requested_at.is_none_or(|t| now - t >= RESIZE_RETRY_SECS) {
            // InnerSize is in points at the zoom it's applied with, which is
            // still this frame's (set_zoom_factor applies next frame).
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_px / ctx.pixels_per_point()));
            fit.resize_requested_at = Some(now);
        }
        // The next frame's input is gathered with the zoom in effect now (a
        // zoom set below only applies once that frame starts).
        fit.input_zoom = Some(ctx.zoom_factor());
        if (zoom - ctx.zoom_factor()).abs() > 0.0001 {
            ctx.set_zoom_factor(zoom);
        }
        // Centered, snapped to whole pixels.
        let min = (ctx.screen_rect().center() - WINDOW_SIZE / 2.0) * ppp;
        egui::Rect::from_min_size(egui::pos2(min.x.round(), min.y.round()) / ppp, WINDOW_SIZE)
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let window = self.fit_window(ctx);
        match self.screen {
            // An update offer (character select only) takes the keyboard
            // while it's up.
            Screen::CharSelect if self.updater.active() => {
                if self.updater.handle_input(ctx) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Screen::CharSelect => {
                self.char_select.handle_input(ctx);
                if self.char_select.confirmed {
                    let now = ctx.input(|i| i.time);
                    self.test_your_might.start_match(ctx, self.char_select.selected_character(), now);
                    self.screen = Screen::TestYourMight;
                }
            }
            Screen::TestYourMight => {
                if self.test_your_might.handle_input(ctx) == test_your_might::Action::CharacterSelect {
                    self.char_select.reopen(self.test_your_might.progress());
                    self.screen = Screen::CharSelect;
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                if let Some(tex) = &self.bg_texture {
                    ui.painter().image(
                        tex.id(),
                        window,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }

                let screen_rect = egui::Rect::from_min_max(SCREEN_MIN, SCREEN_MAX).translate(window.min.to_vec2());
                let mut screen_ui = ui.new_child(egui::UiBuilder::new().max_rect(screen_rect));
                screen_ui.set_clip_rect(screen_rect);
                match self.screen {
                    Screen::CharSelect => {
                        self.char_select.draw(&mut screen_ui, ctx, screen_rect);
                        self.updater.draw(&screen_ui, screen_rect);
                    }
                    Screen::TestYourMight => {
                        self.test_your_might.draw(&mut screen_ui, ctx, screen_rect);
                    }
                }

                let close_rect = egui::Rect::from_min_size(
                    window.min + egui::vec2(WINDOW_W - 30.0, 6.0),
                    egui::vec2(24.0, 24.0),
                );
                let close_resp = ui.interact(
                    close_rect,
                    egui::Id::new("close-button"),
                    egui::Sense::click(),
                );
                let close_color = if close_resp.hovered() {
                    egui::Color32::from_rgb(240, 80, 80)
                } else {
                    egui::Color32::from_rgb(230, 230, 230)
                };
                let painter = ui.painter();
                painter.circle_filled(close_rect.center(), 11.0, egui::Color32::from_black_alpha(140));
                let inset = close_rect.shrink(7.0);
                painter.line_segment([inset.left_top(), inset.right_bottom()], egui::Stroke::new(2.0f32, close_color));
                painter.line_segment([inset.right_top(), inset.left_bottom()], egui::Stroke::new(2.0f32, close_color));
                if close_resp.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                // Drag the whole (undecorated) window from anywhere on the
                // cabinet art outside the screen and the close button.
                if ui.input(|i| i.pointer.primary_pressed()) {
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        if !screen_rect.contains(pos) && !close_rect.contains(pos) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                    }
                }
            });
        self.dev_screenshot.update(ctx);
    }
}

// Release builds on Windows are GUI apps with no console of their own, so
// output from command-line flags would go nowhere. Attach to the console of
// the terminal that launched us, if any (a no-op when started from Explorer,
// and in debug builds, which already have a console).
#[cfg(windows)]
fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // SAFETY: plain Win32 call with a constant argument; failure is harmless.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn main() -> eframe::Result<()> {
    #[cfg(windows)]
    attach_parent_console();

    // Command-line flags:
    //   --version (-v)     print the version and exit
    //   --update           install the latest release, if newer, and exit
    //   --reset            delete the saved progress, then start fresh
    //   --height <frac>    window height as a fraction of the monitor's
    //                      height (default 0.5)
    // --version is checked first so it never has side effects like --reset.
    if std::env::args().skip(1).any(|a| matches!(a.as_str(), "--version" | "--v" | "-v" | "-V")) {
        println!("Test Your Might {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if std::env::args().skip(1).any(|a| a == "--update") {
        std::process::exit(update::run_cli());
    }
    let mut height_fraction = HEIGHT_FRACTION;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--height" => match args.next().and_then(|v| v.parse::<f32>().ok()) {
                Some(f) if (0.1..=1.0).contains(&f) => height_fraction = f,
                _ => eprintln!("--height needs a fraction between 0.1 and 1.0; using {HEIGHT_FRACTION}"),
            },
            "--reset" => match progress::Progress::delete_save() {
                Ok(Some(path)) => eprintln!("--reset: deleted {}", path.display()),
                Ok(None) => eprintln!("--reset: no saved progress to delete"),
                Err(e) => eprintln!("--reset: couldn't delete saved progress: {e}"),
            },
            other => eprintln!("ignoring unknown argument {other:?} (supported: --version, --update, --reset, --height <frac>)"),
        }
    }

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([WINDOW_W, WINDOW_H])
        .with_resizable(false)
        .with_decorations(false)
        .with_always_on_top()
        .with_transparent(false);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Test Your Might",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, height_fraction)))),
    )
}
