// Release builds are a windowed app on Windows: no console window behind the
// game. Debug builds keep the console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Test Your Might - arcade cabinet app scaffold.
// Window is fixed at 500x700 to match mk-cabinet.png (kept that way across
// monitor scale changes, see fit_window). Each screen (character
// select, then Test Your Might with the typing test on its floor) renders only
// inside the cabinet's "screen" rect of that image: (32,151) -> (467,441)
// inclusive, 436x291. See char_select.rs, test_your_might.rs, and
// typing_test.rs for the individual screens.

mod char_select;
mod dev;
mod fighter;
mod progress;
mod test_your_might;
mod typing_test;

use eframe::egui;
use std::time::Duration;

use char_select::CharSelectScreen;
use test_your_might::TestYourMightScreen;

const WINDOW_W: f32 = 500.0;
const WINDOW_H: f32 = 700.0;
const WINDOW_SIZE: egui::Vec2 = egui::vec2(WINDOW_W, WINDOW_H);

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
    // Window size (physical pixels) we last asked the OS to correct, so a
    // refused resize isn't re-requested every frame.
    resize_requested_for: Option<egui::Vec2>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let bg_texture = load_texture(&cc.egui_ctx, "cabinet-bg", include_bytes!("../assets/mk-cabinet.png"));
        let test_your_might = TestYourMightScreen::new(cc);
        let mut app = Self {
            bg_texture,
            screen: Screen::CharSelect,
            char_select: CharSelectScreen::new(cc, test_your_might.progress()),
            test_your_might,
            dev_screenshot: dev::Screenshot::from_env(),
            resize_requested_for: None,
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

    // Moving the window to a monitor with a different scale factor can leave
    // it at a size that no longer matches 500x700 points (the OS/winit resize
    // on a DPI change isn't reliable - seen with mixed-scale monitors on
    // Windows, and reproducible on X11). Everything here is laid out in fixed
    // points, so that used to stretch the cabinet art over the wrong-sized
    // window while the screen and close button stayed put. Ask for the
    // design size back, and until the window has it (or if the OS refuses),
    // zoom so the whole design fits the window as it is. Returns where the
    // 500x700 design sits in the window, in points.
    fn fit_window(&mut self, ctx: &egui::Context) -> egui::Rect {
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        let window_px = ctx.screen_rect().size() * ctx.pixels_per_point();
        let design_px = WINDOW_SIZE * native;
        let off = (window_px - design_px).abs().max_elem() > 1.5;
        if off && self.resize_requested_for != Some(window_px) {
            // InnerSize is in points at the zoom the command is applied with.
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(WINDOW_SIZE / ctx.zoom_factor()));
            self.resize_requested_for = Some(window_px);
        } else if !off {
            self.resize_requested_for = None;
        }
        // Takes effect next frame; this frame keeps the current zoom.
        let fit = (window_px.x / design_px.x).min(window_px.y / design_px.y);
        if (fit - ctx.zoom_factor()).abs() > 0.001 {
            ctx.set_zoom_factor(fit);
        }
        // Centered, snapped to whole pixels so the art isn't resampled.
        let ppp = ctx.pixels_per_point();
        let min = (ctx.screen_rect().center() - WINDOW_SIZE / 2.0) * ppp;
        egui::Rect::from_min_size(egui::pos2(min.x.round(), min.y.round()) / ppp, WINDOW_SIZE)
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let window = self.fit_window(ctx);
        match self.screen {
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

fn main() -> eframe::Result<()> {
    // Command-line flags:
    //   --reset  delete the saved progress, then start fresh
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--reset" => match progress::Progress::delete_save() {
                Ok(Some(path)) => eprintln!("--reset: deleted {}", path.display()),
                Ok(None) => eprintln!("--reset: no saved progress to delete"),
                Err(e) => eprintln!("--reset: couldn't delete saved progress: {e}"),
            },
            other => eprintln!("ignoring unknown argument {other:?} (supported: --reset)"),
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
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
