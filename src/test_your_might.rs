// "Test Your Might" screen: the slab-break minigame backdrop, with the two
// fighter foot-anchor spots (player 1 / player 2-CPU) confirmed via the
// Johnny Cage and Sonya Blade placement sprites.

use eframe::egui;

use crate::load_texture;

// Fighter foot-anchor points on this screen (bottom-center, in tym-bg.png
// pixel coordinates). Every character drawn here uses one of these two spots
// - player 1 (left) or player 2/CPU (right) - always at its native pixel
// size, never resized.
#[allow(dead_code)]
pub const PLAYER1_FOOT: egui::Pos2 = egui::pos2(126.0, 219.0);
#[allow(dead_code)]
pub const PLAYER2_FOOT: egui::Pos2 = egui::pos2(297.0, 219.0);

pub struct TestYourMightScreen {
    tym_bg_texture: Option<egui::TextureHandle>,
}

impl TestYourMightScreen {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let tym_bg_texture = load_texture(&cc.egui_ctx, "tym-bg", include_bytes!("../assets/tym-bg.png"));
        Self { tym_bg_texture }
    }

    pub fn draw(&self, ui: &mut egui::Ui, screen_rect: egui::Rect) {
        if let Some(tex) = &self.tym_bg_texture {
            ui.painter().image(
                tex.id(),
                screen_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }
}
