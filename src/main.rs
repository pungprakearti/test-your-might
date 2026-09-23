// Test Your Might - arcade cabinet app scaffold.
// Window is fixed at 500x700 to match mk-cabinet.png. Each screen (character
// select, test-your-might, typing test) renders only inside the cabinet's
// "screen" rect of that image: (32,151) -> (467,441). See char_select.rs,
// test_your_might.rs, and typing_test.rs for the individual screens.

mod char_select;
mod test_your_might;
mod typing_test;

use eframe::egui;
use std::time::Duration;

use char_select::CharSelectScreen;
use test_your_might::TestYourMightScreen;
use typing_test::TypingScreen;

const WINDOW_W: f32 = 500.0;
const WINDOW_H: f32 = 700.0;

// Screen rect measured from mk-cabinet.png (flood-filled bounding box of the
// blue-gray panel).
const SCREEN_MIN: egui::Pos2 = egui::pos2(32.0, 151.0);
const SCREEN_MAX: egui::Pos2 = egui::pos2(467.0, 441.0);

#[derive(PartialEq)]
enum Screen {
    CharSelect,
    TestYourMight,
    // Not reachable yet - nothing transitions here until the Test Your Might
    // minigame is built to hand off into the typing test.
    #[allow(dead_code)]
    Typing,
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
    typing: TypingScreen,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let bg_texture = load_texture(&cc.egui_ctx, "cabinet-bg", include_bytes!("../assets/mk-cabinet.png"));
        Self {
            bg_texture,
            screen: Screen::CharSelect,
            char_select: CharSelectScreen::new(cc),
            test_your_might: TestYourMightScreen::new(cc),
            typing: TypingScreen::new(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        match self.screen {
            Screen::CharSelect => {
                self.char_select.handle_input(ctx);
                if self.char_select.confirmed {
                    self.screen = Screen::TestYourMight;
                }
            }
            Screen::TestYourMight => {}
            Screen::Typing => {
                self.typing.handle_input(ctx);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                if let Some(tex) = &self.bg_texture {
                    ui.painter().image(
                        tex.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }

                let screen_rect = egui::Rect::from_min_max(SCREEN_MIN, SCREEN_MAX);
                let mut screen_ui = ui.new_child(egui::UiBuilder::new().max_rect(screen_rect));
                screen_ui.set_clip_rect(screen_rect);
                match self.screen {
                    Screen::CharSelect => {
                        self.char_select.draw(&mut screen_ui, ctx, screen_rect);
                    }
                    Screen::TestYourMight => {
                        self.test_your_might.draw(&mut screen_ui, screen_rect);
                    }
                    Screen::Typing => {
                        self.typing.draw(&mut screen_ui, ctx, screen_rect);
                    }
                }

                let close_rect = egui::Rect::from_min_size(
                    egui::pos2(WINDOW_W - 30.0, 6.0),
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
    }
}

fn main() -> eframe::Result<()> {
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
