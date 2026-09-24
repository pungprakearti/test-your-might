// Character select screen: the 7-portrait cross-shaped grid drawn over
// cs-background.png, navigated with arrow keys, with a blinking green
// selector. Locked fighters (see `Character::unlock`) can't be picked; a line
// under the grid says who's highlighted and how to unlock them. The theme
// loops while choosing (started by the app); moving the cursor clicks;
// confirming stops the theme, plays a music cue and holds on the pick for
// CONFIRM_HOLD_SECS before the match starts.

use eframe::egui;

use crate::audio::{Audio, Sound};
use crate::fighter::{Character, Unlock};
use crate::load_texture;
use crate::progress::Progress;

// Character select grid, positions measured (center point, in cs-background.png
// pixel coordinates) from the portrait cells in that image. The grid is a
// cross shape: row 0 has 4 portraits (cols 1,2,4,5 - col 3 is the dragon
// logo and isn't selectable), row 1 has 3 portraits (cols 2,3,4).
struct CharCell {
    character: Character,
    center: egui::Pos2,
    row: u8,
}

const CHAR_CELLS: [CharCell; 7] = [
    CharCell { character: Character::JohnnyCage, center: egui::pos2(80.0, 98.5), row: 0 },
    CharCell { character: Character::Kano, center: egui::pos2(149.0, 98.5), row: 0 },
    CharCell { character: Character::SubZero, center: egui::pos2(282.5, 98.5), row: 0 },
    CharCell { character: Character::SonyaBlade, center: egui::pos2(349.5, 98.5), row: 0 },
    CharCell { character: Character::Raiden, center: egui::pos2(149.0, 181.0), row: 1 },
    CharCell { character: Character::LiuKang, center: egui::pos2(216.5, 181.0), row: 1 },
    CharCell { character: Character::Scorpion, center: egui::pos2(282.5, 181.0), row: 1 },
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
// How long the screen holds on the chosen fighter after Enter.
const CONFIRM_HOLD_SECS: f64 = 1.0;

// The line under the grid naming the highlighted fighter and how to unlock
// them: centered in the plain stone below the grid frame (which ends at
// y=236 in cs-background.png), on a dark pill for readability.
const HINT_CENTER: egui::Pos2 = egui::pos2(218.0, 264.0);
const HINT_FONT: f32 = 13.0;
const HINT_PAD: egui::Vec2 = egui::vec2(10.0, 4.0);
const HINT_BACKDROP_ALPHA: u8 = 170;
const HINT_READY: egui::Color32 = egui::Color32::from_rgb(120, 220, 140);
const HINT_LOCKED: egui::Color32 = egui::Color32::from_gray(200);

pub struct CharSelectScreen {
    cs_bg_texture: Option<egui::TextureHandle>,
    cs_sel1_texture: Option<egui::TextureHandle>,
    cs_sel2_texture: Option<egui::TextureHandle>,
    // Per-CHAR_CELLS-index unlocked-portrait overlay (cs-<name>.png). Liu
    // Kang has no entry (his portrait is already baked into cs-background.png).
    cs_portrait_textures: [Option<egui::TextureHandle>; 7],
    // Per-CHAR_CELLS-index unlock state, and the days played (for
    // Scorpion's hint), refreshed from saved progress.
    unlocked: [bool; 7],
    days_played: usize,
    selected: usize,
    // egui time Enter confirmed the selected fighter.
    confirmed_at: Option<f64>,
}

impl CharSelectScreen {
    pub fn new(cc: &eframe::CreationContext<'_>, progress: &Progress) -> Self {
        let cs_bg_texture = load_texture(&cc.egui_ctx, "cs-bg", include_bytes!("../assets/cs-background.png"));
        let cs_sel1_texture = load_texture(&cc.egui_ctx, "cs-sel1", include_bytes!("../assets/cs-selected-1.png"));
        let cs_sel2_texture = load_texture(&cc.egui_ctx, "cs-sel2", include_bytes!("../assets/cs-selected-2.png"));
        // Indices match CHAR_CELLS: Johnny Cage, Kano, Sub-Zero, Sonya Blade,
        // Raiden, Liu Kang (none), Scorpion.
        let cs_portrait_textures = [
            load_texture(&cc.egui_ctx, "cs-johnnycage", include_bytes!("../assets/cs-johnnycage.png")),
            load_texture(&cc.egui_ctx, "cs-kano", include_bytes!("../assets/cs-kano.png")),
            load_texture(&cc.egui_ctx, "cs-subzero", include_bytes!("../assets/cs-subzero.png")),
            load_texture(&cc.egui_ctx, "cs-sonyablade", include_bytes!("../assets/cs-sonyablade.png")),
            load_texture(&cc.egui_ctx, "cs-raiden", include_bytes!("../assets/cs-raiden.png")),
            None,
            load_texture(&cc.egui_ctx, "cs-scorpion", include_bytes!("../assets/cs-scorpion.png")),
        ];
        let mut screen = Self {
            cs_bg_texture,
            cs_sel1_texture,
            cs_sel2_texture,
            cs_portrait_textures,
            unlocked: [false; 7],
            days_played: 0,
            selected: 0,
            confirmed_at: None,
        };
        screen.refresh_unlocks(progress);
        // Start on a fighter that can actually be picked.
        screen.selected = (0..CHAR_CELLS.len()).find(|&i| screen.unlocked[i]).unwrap_or(0);
        screen
    }

    // Back from a match: pick again, starting from the last choice, with
    // anything unlocked since then now available.
    pub fn reopen(&mut self, progress: &Progress) {
        self.confirmed_at = None;
        self.refresh_unlocks(progress);
    }

    fn refresh_unlocks(&mut self, progress: &Progress) {
        self.unlocked = std::array::from_fn(|i| progress.is_unlocked(CHAR_CELLS[i].character));
        self.days_played = progress.days_played();
    }

    fn hint(&self) -> (String, egui::Color32) {
        let character = CHAR_CELLS[self.selected].character;
        let name = character.name();
        if self.unlocked[self.selected] {
            return (format!("{name}  -  ENTER to fight"), HINT_READY);
        }
        let how = match character.unlock() {
            Unlock::Default => unreachable!("default fighters are always unlocked"),
            Unlock::Break(material) => format!("break {} to unlock", material.label()),
            Unlock::PlayedDays(days) => format!("play {days} different days ({}/{days})", self.days_played.min(days)),
        };
        (format!("{name}  -  {how}"), HINT_LOCKED)
    }

    // Moves the cursor onto `character` (dev hook).
    pub fn dev_select(&mut self, character: Character) {
        if let Some(i) = CHAR_CELLS.iter().position(|c| c.character == character) {
            self.selected = i;
        }
    }

    pub fn selected_character(&self) -> Character {
        CHAR_CELLS[self.selected].character
    }

    // The fighter to start a match with, once the hold after confirming is
    // over.
    pub fn chosen(&self, ctx: &egui::Context) -> Option<Character> {
        let left = self.confirmed_at? + CONFIRM_HOLD_SECS - ctx.input(|i| i.time);
        if left > 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(left));
            return None;
        }
        Some(self.selected_character())
    }

    pub fn handle_input(&mut self, ctx: &egui::Context, audio: &mut Audio) {
        if self.confirmed_at.is_some() {
            return;
        }
        let before = self.selected;
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
            if i.key_pressed(egui::Key::Enter) && self.unlocked[self.selected] {
                self.confirmed_at = Some(i.time);
            }
        });
        if self.confirmed_at.is_some() {
            audio.stop_music();
            audio.play(Sound::FighterChosen);
        } else if self.selected != before {
            audio.play(Sound::CursorMove);
        }
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
            if !self.unlocked[i] {
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
        let use_frame1 = self.confirmed_at.is_some() || ((time / SELECTOR_ANIM_FRAME_SECS) as u64).is_multiple_of(2);
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

        let (text, color) = self.hint();
        let painter = ui.painter();
        let galley = painter.layout_no_wrap(text, egui::FontId::monospace(HINT_FONT), color);
        let pill = egui::Rect::from_center_size(screen_rect.min + HINT_CENTER.to_vec2(), galley.size() + HINT_PAD * 2.0);
        painter.rect_filled(pill, 4.0, egui::Color32::from_black_alpha(HINT_BACKDROP_ALPHA));
        painter.galley(pill.min + HINT_PAD, galley, color);
    }
}
