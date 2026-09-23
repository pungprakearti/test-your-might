// Character select screen: the 7-portrait cross-shaped grid drawn over
// cs-background.png, navigated with arrow keys, with a blinking green
// selector and per-character unlock flags.

use eframe::egui;

use crate::fighter::Character;
use crate::load_texture;

// WPM tiers a character can unlock at, as a fraction of the player's average
// WPM across their first 3 30s tests (25/50/75/100/105%). Averaging + tier
// evaluation isn't built yet (see PROJECT.md "Not yet built"), so for now
// these are just flags describing each character's unlock rule.
#[derive(Clone, Copy)]
pub enum WpmTier {
    Wood,
    Stone,
    Iron,
    Ruby,
    Diamond,
}

// The rule that unlocks a character. `is_unlocked` below is a placeholder
// until WPM-tier averaging and played-day tracking exist to evaluate these
// for real - today only `Default` characters are actually unlocked.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum UnlockCondition {
    Default,
    WpmTier(WpmTier),
    PlayedDays(u32),
}

fn is_unlocked(cond: UnlockCondition) -> bool {
    match cond {
        UnlockCondition::Default => true,
        UnlockCondition::WpmTier(_) | UnlockCondition::PlayedDays(_) => false,
    }
}

// Character select grid, positions measured (center point, in cs-background.png
// pixel coordinates) from the portrait cells in that image. The grid is a
// cross shape: row 0 has 4 portraits (cols 1,2,4,5 - col 3 is the dragon
// logo and isn't selectable), row 1 has 3 portraits (cols 2,3,4).
struct CharCell {
    character: Character,
    center: egui::Pos2,
    row: u8,
    unlock: UnlockCondition,
}

impl CharCell {
    fn locked(&self) -> bool {
        !is_unlocked(self.unlock)
    }
}

const CHAR_CELLS: [CharCell; 7] = [
    CharCell { character: Character::JohnnyCage, center: egui::pos2(80.0, 98.5), row: 0, unlock: UnlockCondition::WpmTier(WpmTier::Wood) },
    CharCell { character: Character::Kano, center: egui::pos2(149.0, 98.5), row: 0, unlock: UnlockCondition::WpmTier(WpmTier::Stone) },
    CharCell { character: Character::Scorpion, center: egui::pos2(282.5, 98.5), row: 0, unlock: UnlockCondition::PlayedDays(10) },
    CharCell { character: Character::SonyaBlade, center: egui::pos2(349.5, 98.5), row: 0, unlock: UnlockCondition::WpmTier(WpmTier::Ruby) },
    CharCell { character: Character::Raiden, center: egui::pos2(149.0, 181.0), row: 1, unlock: UnlockCondition::WpmTier(WpmTier::Iron) },
    CharCell { character: Character::LiuKang, center: egui::pos2(216.5, 181.0), row: 1, unlock: UnlockCondition::Default },
    CharCell { character: Character::SubZero, center: egui::pos2(282.5, 181.0), row: 1, unlock: UnlockCondition::WpmTier(WpmTier::Diamond) },
];

const ROW0: [usize; 4] = [0, 1, 2, 3];
const ROW1: [usize; 3] = [4, 5, 6];

fn row_of(idx: usize) -> &'static [usize] {
    if CHAR_CELLS[idx].row == 0 { &ROW0 } else { &ROW1 }
}

fn move_horizontal(idx: usize, delta: i32) -> usize {
    let row = row_of(idx);
    let pos = row.iter().position(|&i| i == idx).unwrap() as i32;
    let len = row.len() as i32;
    let new_pos = ((pos + delta).rem_euclid(len)) as usize;
    row[new_pos]
}

fn move_vertical(idx: usize, target_row: u8) -> usize {
    if CHAR_CELLS[idx].row == target_row {
        return idx;
    }
    let target: &[usize] = if target_row == 0 { &ROW0 } else { &ROW1 };
    let cur_x = CHAR_CELLS[idx].center.x;
    *target
        .iter()
        .min_by(|&&a, &&b| {
            let da = (CHAR_CELLS[a].center.x - cur_x).abs();
            let db = (CHAR_CELLS[b].center.x - cur_x).abs();
            da.partial_cmp(&db).unwrap()
        })
        .unwrap()
}

const SELECTOR_ANIM_FRAME_SECS: f64 = 0.25;

pub struct CharSelectScreen {
    cs_bg_texture: Option<egui::TextureHandle>,
    cs_sel1_texture: Option<egui::TextureHandle>,
    cs_sel2_texture: Option<egui::TextureHandle>,
    // Per-CHAR_CELLS-index unlocked-portrait overlay (cs-<name>.png). Liu
    // Kang has no entry (his portrait is already baked into cs-background.png).
    cs_portrait_textures: [Option<egui::TextureHandle>; 7],
    selected: usize,
    pub confirmed: bool,
}

impl CharSelectScreen {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let cs_bg_texture = load_texture(&cc.egui_ctx, "cs-bg", include_bytes!("../assets/cs-background.png"));
        let cs_sel1_texture = load_texture(&cc.egui_ctx, "cs-sel1", include_bytes!("../assets/cs-selected-1.png"));
        let cs_sel2_texture = load_texture(&cc.egui_ctx, "cs-sel2", include_bytes!("../assets/cs-selected-2.png"));
        // Indices match CHAR_CELLS: Johnny Cage, Kano, Scorpion, Sonya Blade,
        // Raiden, Liu Kang (none), Sub-Zero.
        let cs_portrait_textures = [
            load_texture(&cc.egui_ctx, "cs-johnnycage", include_bytes!("../assets/cs-johnnycage.png")),
            load_texture(&cc.egui_ctx, "cs-kano", include_bytes!("../assets/cs-kano.png")),
            load_texture(&cc.egui_ctx, "cs-scorpion", include_bytes!("../assets/cs-scorpion.png")),
            load_texture(&cc.egui_ctx, "cs-sonyablade", include_bytes!("../assets/cs-sonyablade.png")),
            load_texture(&cc.egui_ctx, "cs-raiden", include_bytes!("../assets/cs-raiden.png")),
            None,
            load_texture(&cc.egui_ctx, "cs-subzero", include_bytes!("../assets/cs-subzero.png")),
        ];
        Self {
            cs_bg_texture,
            cs_sel1_texture,
            cs_sel2_texture,
            cs_portrait_textures,
            selected: 0,
            confirmed: false,
        }
    }

    pub fn selected_character(&self) -> Character {
        CHAR_CELLS[self.selected].character
    }

    pub fn handle_input(&mut self, ctx: &egui::Context) {
        if self.confirmed {
            return;
        }
        ctx.input(|i| {
            if i.key_pressed(egui::Key::ArrowLeft) {
                self.selected = move_horizontal(self.selected, -1);
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                self.selected = move_horizontal(self.selected, 1);
            }
            if i.key_pressed(egui::Key::ArrowUp) {
                self.selected = move_vertical(self.selected, 0);
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                self.selected = move_vertical(self.selected, 1);
            }
            if i.key_pressed(egui::Key::Enter) && !CHAR_CELLS[self.selected].locked() {
                self.confirmed = true;
            }
        });
    }

    pub fn draw(&self, ui: &mut egui::Ui, ctx: &egui::Context, screen_rect: egui::Rect) {
        if let Some(tex) = &self.cs_bg_texture {
            ui.painter().image(
                tex.id(),
                screen_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        for (i, cell) in CHAR_CELLS.iter().enumerate() {
            if cell.locked() {
                continue;
            }
            if let Some(tex) = &self.cs_portrait_textures[i] {
                let portrait_rect = egui::Rect::from_center_size(
                    screen_rect.min + cell.center.to_vec2(),
                    egui::vec2(59.0, 74.0),
                );
                ui.painter().image(
                    tex.id(),
                    portrait_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
        let cell = &CHAR_CELLS[self.selected];
        let time = ctx.input(|i| i.time);
        let use_frame1 = self.confirmed || ((time / SELECTOR_ANIM_FRAME_SECS) as u64).is_multiple_of(2);
        let sel_tex = if use_frame1 { &self.cs_sel1_texture } else { &self.cs_sel2_texture };
        if let Some(tex) = sel_tex {
            let sel_rect = egui::Rect::from_center_size(
                screen_rect.min + cell.center.to_vec2(),
                egui::vec2(67.0, 82.0),
            );
            ui.painter().image(
                tex.id(),
                sel_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }
}
