// "Test Your Might" screen: the slab-break minigame backdrop, with the two
// fighter foot-anchor spots (player 1 / player 2-CPU) confirmed via the
// Johnny Cage and Sonya Blade placement sprites. Player 1 is the character
// picked on the select screen; player 2 is a random different character.

use eframe::egui;

use crate::fighter::{Character, Fighter};
use crate::load_texture;

// Fighter foot-anchor points on this screen (bottom-center, in tym-bg.png
// pixel coordinates). Every character drawn here uses one of these two spots
// - player 1 (left) or player 2/CPU (right) - always at its native pixel
// size, never resized.
pub const PLAYER1_FOOT: egui::Pos2 = egui::pos2(126.0, 219.0);
pub const PLAYER2_FOOT: egui::Pos2 = egui::pos2(297.0, 219.0);

// Top-left of each player's breakable material (tym-bg.png pixel
// coordinates). Each lines up exactly with a 182x58 green bracket box baked
// into tym-bg.png - P1 (36,119)-(217,176), P2 (210,119)-(391,176) inclusive -
// which matches the size of material-placement.png and the tym-<material>-N.png
// sprites. Confirmed correct by the user for both players. Drawn at native
// size (never resized), in front of the fighters.
pub const PLAYER1_MATERIAL: egui::Pos2 = egui::pos2(36.0, 119.0);
pub const PLAYER2_MATERIAL: egui::Pos2 = egui::pos2(210.0, 119.0);

pub struct TestYourMightScreen {
    tym_bg_texture: Option<egui::TextureHandle>,
    material_texture: Option<egui::TextureHandle>,
    // (player 1, player 2/CPU); None until a character is confirmed.
    fighters: Option<(Fighter, Fighter)>,
}

impl TestYourMightScreen {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let tym_bg_texture = load_texture(&cc.egui_ctx, "tym-bg", include_bytes!("../assets/tym-bg.png"));
        let material_texture = load_texture(&cc.egui_ctx, "tym-wood-1", include_bytes!("../assets/tym-wood-1.png"));
        Self { tym_bg_texture, material_texture, fighters: None }
    }

    pub fn start_match(&mut self, ctx: &egui::Context, player: Character, now: f64) {
        let cpu = player.random_opponent();
        self.fighters = Some((Fighter::new(ctx, player, now), Fighter::new(ctx, cpu, now)));
    }

    pub fn draw(&self, ui: &mut egui::Ui, ctx: &egui::Context, screen_rect: egui::Rect) {
        if let Some(tex) = &self.tym_bg_texture {
            ui.painter().image(
                tex.id(),
                screen_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        if let Some((p1, p2)) = &self.fighters {
            let now = ctx.input(|i| i.time);
            let painter = ui.painter();
            p1.draw(painter, screen_rect.min + PLAYER1_FOOT.to_vec2(), now);
            p2.draw(painter, screen_rect.min + PLAYER2_FOOT.to_vec2(), now);
            if let Some(tex) = &self.material_texture {
                for pos in [PLAYER1_MATERIAL, PLAYER2_MATERIAL] {
                    let rect = egui::Rect::from_min_size(screen_rect.min + pos.to_vec2(), tex.size_vec2());
                    painter.image(
                        tex.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
            let next = [p1.next_frame_in(now), p2.next_frame_in(now)].into_iter().flatten().reduce(f64::min);
            if let Some(secs) = next {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(secs));
            }
        }
    }
}
