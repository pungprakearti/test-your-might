// "Test Your Might" screen: the slab-break minigame backdrop, with the two
// fighter foot-anchor spots (player 1 / player 2-CPU) confirmed via the
// Johnny Cage and Sonya Blade placement sprites. Player 1 is the character
// picked on the select screen; player 2 is a random different character. The
// typing test runs on the concrete floor strip along the bottom.

use eframe::egui;

use crate::fighter::{Character, Fighter, Pose};
use crate::load_texture;
use crate::progress::{Material, Progress, Run};
use crate::typing_test::{self, TypingScreen};

// Fighter foot-anchor points on this screen (bottom-center, in tym-bg.png
// pixel coordinates). Every character drawn here uses one of these two spots
// - player 1 (left) or player 2/CPU (right) - always at its native pixel
// size, never resized.
pub const PLAYER1_FOOT: egui::Pos2 = egui::pos2(126.0, 219.0);
pub const PLAYER2_FOOT: egui::Pos2 = egui::pos2(297.0, 219.0);

// Top-left of each player's breakable material (tym-bg.png pixel
// coordinates). Each lines up exactly with a 182x58 green bracket box that
// was marked in tym-bg.png (markers since removed; see v0.0.9 in git) - P1 (36,119)-(217,176), P2 (210,119)-(391,176) inclusive -
// which matches the size of material-placement.png and the tym-<material>-N.png
// sprites. Confirmed correct by the user for both players. Drawn at native
// size (never resized), in front of the fighters.
pub const PLAYER1_MATERIAL: egui::Pos2 = egui::pos2(36.0, 119.0);
pub const PLAYER2_MATERIAL: egui::Pos2 = egui::pos2(210.0, 119.0);

// Top-left of each player's gauge (tym-bg.png pixel coordinates), one on the
// outer side of each fighter. Each lines up exactly with a 33x170 box of
// orange-red corner markers that were in tym-bg.png (since removed) - P1 (39,2)-(71,171), P2
// (364,2)-(396,171) inclusive - which matches guage.png's size. Drawn at
// native size, behind the fighters. Placement confirmed by the user.
pub const PLAYER1_GAUGE: egui::Pos2 = egui::pos2(39.0, 2.0);
pub const PLAYER2_GAUGE: egui::Pos2 = egui::pos2(364.0, 2.0);

// The gauge's see-through window, in guage.png pixel coordinates: 25x160
// inside the 4px frame (5px at the top/bottom edges). The yellow speed fill
// is drawn behind the gauge inside this window, rising from the bottom, and
// is clamped to it.
const GAUGE_WINDOW_MIN: egui::Pos2 = egui::pos2(4.0, 5.0);
const GAUGE_WINDOW_SIZE: egui::Vec2 = egui::vec2(25.0, 160.0);
const GAUGE_FILL_COLOR: egui::Color32 = egui::Color32::from_rgb(248, 216, 0);
// The red target line: drawn on top of the gauge, across its full width, at
// the current material's bar height.
const TARGET_BAR_COLOR: egui::Color32 = egui::Color32::from_rgb(230, 30, 30);
const TARGET_BAR_THICKNESS: f32 = 2.0;
// How quickly the drawn fill chases the actual speed (per second); higher is
// snappier, lower is smoother.
const GAUGE_EASE_RATE: f32 = 8.0;
// The beat between a slab breaking and the victory animation starting.
const VICTORY_DELAY_SECS: f64 = 0.6;

// What the Test Your Might screen asks the app to do after handling input.
#[derive(PartialEq)]
pub enum Action {
    Stay,
    // Esc: go back to character select.
    CharacterSelect,
}

// Text colors for the end-of-round panel.
const RESULT_GOOD: egui::Color32 = egui::Color32::from_rgb(120, 220, 140);
const RESULT_BAD: egui::Color32 = egui::Color32::from_rgb(235, 70, 70);
const RESULT_GOLD: egui::Color32 = egui::Color32::from_rgb(240, 200, 40);
const RESULT_TEXT: egui::Color32 = egui::Color32::from_gray(200);

// The concrete floor strip the typing test sits on (tym-bg.png pixel
// coordinates): full width, from just under the 1px highlight line at row 207
// down to the bottom edge - 436x83.
const FLOOR_MIN: egui::Pos2 = egui::pos2(0.0, 208.0);
const FLOOR_MAX: egui::Pos2 = egui::pos2(436.0, 291.0);

pub struct TestYourMightScreen {
    tym_bg_texture: Option<egui::TextureHandle>,
    // tym-<material>-1.png (intact) and -2.png (broken), indexed like
    // Material::ALL.
    material_textures: [Option<egui::TextureHandle>; 5],
    broken_material_textures: [Option<egui::TextureHandle>; 5],
    gauge_texture: Option<egui::TextureHandle>,
    // (player 1, player 2/CPU); None until a character is confirmed.
    fighters: Option<(Fighter, Fighter)>,
    typing: TypingScreen,
    progress: Progress,
    round: Round,
    // Currently drawn gauge fills (fraction of the window height), eased
    // toward the real values each frame.
    p1_fill: f32,
    p2_fill: f32,
}

// One 30s test: what's being broken, the target it was set with, and how the
// CPU's gauge will play out.
struct Round {
    material: Material,
    target_wpm: f64,
    cpu: CpuRun,
    // Set once the test finishes and the run has been recorded.
    result: Option<Run>,
    // Fighters this round's result unlocked, for the results panel.
    new_unlocks: Vec<Character>,
    // egui time the timer ran out, which starts the strike sequence.
    ended_at: Option<f64>,
}

// Where one fighter is in the end-of-round sequence: strike when the timer
// runs out; at the strike's impact (last strike frame) the slab breaks if
// they were fast enough; a beat later, the victory animation plays. A fighter
// who fell short holds on the last strike frame.
#[derive(Debug, PartialEq)]
struct Ending {
    pose: Pose,
    // egui time the pose started, so animations run from the right frame.
    pose_started: f64,
    broken: bool,
    // egui time of this fighter's next change, if any.
    next_change: Option<f64>,
}

fn ending_at(ended_at: f64, passed: bool, now: f64) -> Ending {
    let impact = ended_at + Pose::Strike.secs_to_last_frame();
    let victory = impact + VICTORY_DELAY_SECS;
    if passed && now >= victory {
        Ending { pose: Pose::Victory, pose_started: victory, broken: true, next_change: None }
    } else {
        let broken = passed && now >= impact;
        let next_change = match (passed, broken) {
            (false, _) => None,
            (true, false) => Some(impact),
            (true, true) => Some(victory),
        };
        Ending { pose: Pose::Strike, pose_started: ended_at, broken, next_change }
    }
}

impl TestYourMightScreen {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        let tym_bg_texture = load_texture(ctx, "tym-bg", include_bytes!("../assets/tym-bg.png"));
        let material_textures = [
            load_texture(ctx, "tym-wood-1", include_bytes!("../assets/tym-wood-1.png")),
            load_texture(ctx, "tym-stone-1", include_bytes!("../assets/tym-stone-1.png")),
            load_texture(ctx, "tym-steel-1", include_bytes!("../assets/tym-steel-1.png")),
            load_texture(ctx, "tym-ruby-1", include_bytes!("../assets/tym-ruby-1.png")),
            load_texture(ctx, "tym-diamond-1", include_bytes!("../assets/tym-diamond-1.png")),
        ];
        let broken_material_textures = [
            load_texture(ctx, "tym-wood-2", include_bytes!("../assets/tym-wood-2.png")),
            load_texture(ctx, "tym-stone-2", include_bytes!("../assets/tym-stone-2.png")),
            load_texture(ctx, "tym-steel-2", include_bytes!("../assets/tym-steel-2.png")),
            load_texture(ctx, "tym-ruby-2", include_bytes!("../assets/tym-ruby-2.png")),
            load_texture(ctx, "tym-diamond-2", include_bytes!("../assets/tym-diamond-2.png")),
        ];
        let gauge_texture = load_texture(ctx, "guage", include_bytes!("../assets/guage.png"));
        let progress = Progress::load();
        let round = Round::new(&progress);
        Self {
            tym_bg_texture,
            material_textures,
            broken_material_textures,
            gauge_texture,
            fighters: None,
            typing: TypingScreen::new(),
            progress,
            round,
            p1_fill: 0.0,
            p2_fill: 0.0,
        }
    }

    pub fn start_match(&mut self, ctx: &egui::Context, player: Character, now: f64) {
        let cpu = player.random_opponent();
        self.fighters = Some((Fighter::new(ctx, player, now), Fighter::new(ctx, cpu, now)));
        self.start_round(now);
    }

    fn start_round(&mut self, now: f64) {
        if let Some((p1, p2)) = &mut self.fighters {
            p1.set_pose(Pose::Idle, now);
            p2.set_pose(Pose::Idle, now);
        }
        self.typing = TypingScreen::new();
        self.round = Round::new(&self.progress);
        self.p1_fill = 0.0;
        self.p2_fill = 0.0;
    }

    pub fn handle_input(&mut self, ctx: &egui::Context) -> Action {
        let now = ctx.input(|i| i.time);
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return Action::CharacterSelect;
        }
        self.typing.handle_input(ctx);
        if self.typing.finished() && self.round.result.is_none() {
            let locked_before: Vec<Character> =
                Character::ALL.into_iter().filter(|&c| !self.progress.is_unlocked(c)).collect();
            let run = self.progress.record(self.typing.wpm(), self.typing.accuracy(), self.round.target_wpm);
            self.progress.save();
            self.round.new_unlocks = locked_before.into_iter().filter(|&c| self.progress.is_unlocked(c)).collect();
            self.round.cpu.resolve_diamond(run.passed);
            self.round.result = Some(run);
            self.round.ended_at = Some(now);
        } else if self.round.result.is_some()
            && ctx.input(|i| i.key_pressed(egui::Key::Enter))
        {
            self.start_round(now);
            return Action::Stay;
        }
        if let (Some([p1_end, p2_end]), Some((p1, p2))) = (self.endings(now), &mut self.fighters) {
            p1.set_pose(p1_end.pose, p1_end.pose_started);
            p2.set_pose(p2_end.pose, p2_end.pose_started);
            if let Some(next) = [p1_end.next_change, p2_end.next_change].into_iter().flatten().reduce(f64::min) {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64((next - now).max(0.0)));
            }
        }
        Action::Stay
    }

    // Each fighter's end-of-round state, once the timer has run out.
    fn endings(&self, now: f64) -> Option<[Ending; 2]> {
        let ended_at = self.round.ended_at?;
        let p1_passed = self.round.result.as_ref().is_some_and(|r| r.passed);
        Some([ending_at(ended_at, p1_passed, now), ending_at(ended_at, self.round.cpu.passes(), now)])
    }

    pub fn progress(&self) -> &Progress {
        &self.progress
    }

    pub fn dev_type(&mut self, text: &str) {
        self.typing.type_text(text);
    }

    pub fn dev_type_words(&mut self, n: usize) {
        self.typing.type_words(n);
    }

    // Player 1's and the CPU's speed as a multiple of the target (1.0 = right
    // on the red bar).
    fn speed_ratios(&self) -> (f64, f64) {
        if !self.typing.started() {
            return (0.0, 0.0);
        }
        // Once done, show the whole-number WPM that pass/fail was judged on.
        let p1_wpm = if self.typing.finished() { self.typing.wpm().round() } else { self.typing.live_wpm() };
        (p1_wpm / self.round.target_wpm, self.round.cpu.ratio_at(self.typing.elapsed_secs()))
    }

    // Draws the floor strip: the typing test while a round is on, then the
    // round's results and what to do next.
    fn draw_floor(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, floor: egui::Rect) {
        let material = self.round.material.label();
        let Some(run) = &self.round.result else {
            let status = format!("{material}  goal {:.0}", self.round.target_wpm);
            self.typing.draw(ui, ctx, floor, &status, "type to start   ESC choose fighter");
            return;
        };
        let stats = format!("WPM {:.0}   ACC {:.0}%", run.wpm, run.accuracy);
        let goal = format!("goal {:.0}", run.target_wpm);
        let (mut verdict, verdict_color) = if run.passed {
            (format!("{material} BROKEN!"), RESULT_GOLD)
        } else {
            (format!("TOO SLOW - {material} HELD"), RESULT_BAD)
        };
        if !self.round.new_unlocks.is_empty() {
            let names: Vec<&str> = self.round.new_unlocks.iter().map(|c| c.name()).collect();
            verdict += &format!("  {} UNLOCKED!", names.join(" + "));
        }
        // Progress has already recorded this run, so it holds the next round.
        let next_material = self.progress.material.label();
        let next_goal = self.progress.target_wpm();
        let next = if !run.passed && self.round.material != Material::Wood {
            format!("Back to {next_material}, goal {next_goal:.0}")
        } else if self.progress.material == self.round.material {
            format!("Next: {next_material} again, goal {next_goal:.0}")
        } else {
            format!("Next: {next_material}, goal {next_goal:.0}")
        };
        typing_test::draw_panel(
            ui,
            floor,
            (&stats, if run.passed { RESULT_GOOD } else { RESULT_BAD }),
            (&goal, RESULT_GOLD),
            &[(&verdict, verdict_color), (&next, RESULT_TEXT), ("ENTER next round   ESC choose fighter", RESULT_TEXT)],
        );
    }

    // Layers, back to front: background, gauge fills, gauges + target bars,
    // fighters, materials, typing test.
    pub fn draw(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, screen_rect: egui::Rect) {
        let painter = ui.painter().clone();
        let at = |pos: egui::Pos2| screen_rect.min + pos.to_vec2();
        if let Some(tex) = &self.tym_bg_texture {
            draw_sprite(&painter, tex, screen_rect.min, egui::Color32::WHITE);
        }

        let bar = self.round.material.bar_fraction();
        let (p1_ratio, p2_ratio) = self.speed_ratios();
        let ease = 1.0 - (-ctx.input(|i| i.stable_dt) * GAUGE_EASE_RATE).exp();
        self.p1_fill += (fill_fraction(p1_ratio, bar) - self.p1_fill) * ease;
        self.p2_fill += (fill_fraction(p2_ratio, bar) - self.p2_fill) * ease;
        let settled = (fill_fraction(p1_ratio, bar) - self.p1_fill).abs() < 0.001
            && (fill_fraction(p2_ratio, bar) - self.p2_fill).abs() < 0.001;
        if (self.typing.started() && !self.typing.finished()) || !settled {
            ctx.request_repaint();
        }
        for (pos, fill) in [(PLAYER1_GAUGE, self.p1_fill), (PLAYER2_GAUGE, self.p2_fill)] {
            let window = egui::Rect::from_min_size(at(pos) + GAUGE_WINDOW_MIN.to_vec2(), GAUGE_WINDOW_SIZE);
            let h = (fill * GAUGE_WINDOW_SIZE.y).round();
            if h > 0.0 {
                let fill_rect = egui::Rect::from_min_max(egui::pos2(window.min.x, window.max.y - h), window.max);
                painter.rect_filled(fill_rect, 0.0, GAUGE_FILL_COLOR);
            }
        }
        if let Some(tex) = &self.gauge_texture {
            for pos in [PLAYER1_GAUGE, PLAYER2_GAUGE] {
                draw_sprite(&painter, tex, at(pos), egui::Color32::WHITE);
                let window_bottom = at(pos).y + GAUGE_WINDOW_MIN.y + GAUGE_WINDOW_SIZE.y;
                let bar_y = (window_bottom - bar * GAUGE_WINDOW_SIZE.y).round();
                let bar_rect = egui::Rect::from_min_max(
                    egui::pos2(at(pos).x, bar_y - TARGET_BAR_THICKNESS / 2.0),
                    egui::pos2(at(pos).x + tex.size_vec2().x, bar_y + TARGET_BAR_THICKNESS / 2.0),
                );
                painter.rect_filled(bar_rect, 0.0, TARGET_BAR_COLOR);
            }
        }

        if let Some((p1, p2)) = &self.fighters {
            let now = ctx.input(|i| i.time);
            p1.draw(&painter, at(PLAYER1_FOOT), now);
            p2.draw(&painter, at(PLAYER2_FOOT), now);
            let next = [p1.next_frame_in(now), p2.next_frame_in(now)].into_iter().flatten().reduce(f64::min);
            if let Some(secs) = next {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(secs));
            }
        }
        let material_idx = Material::ALL.iter().position(|&m| m == self.round.material).unwrap();
        let now = ctx.input(|i| i.time);
        let broken = self.endings(now).map_or([false, false], |[p1, p2]| [p1.broken, p2.broken]);
        for (pos, broken) in [(PLAYER1_MATERIAL, broken[0]), (PLAYER2_MATERIAL, broken[1])] {
            let textures = if broken { &self.broken_material_textures } else { &self.material_textures };
            if let Some(tex) = &textures[material_idx] {
                draw_sprite(&painter, tex, at(pos), egui::Color32::WHITE);
            }
        }

        let floor = egui::Rect::from_min_max(at(FLOOR_MIN), at(FLOOR_MAX));
        self.draw_floor(ui, ctx, floor);
    }
}

impl Round {
    fn new(progress: &Progress) -> Self {
        let material = progress.material;
        Round { material, target_wpm: progress.target_wpm(), cpu: CpuRun::new(material), result: None, new_unlocks: Vec::new(), ended_at: None }
    }
}

// Gauge fill (fraction of the window height) for a speed `ratio` of the
// target, with the target drawn at `bar` - so ratio 1.0 lands exactly on the
// red bar. Never leaves the gauge.
fn fill_fraction(ratio: f64, bar: f32) -> f32 {
    (ratio as f32 * bar).clamp(0.0, 1.0)
}

// The CPU's scripted gauge for one round. It charges up, then swings over and
// under the red bar the whole test, dips under it a few seconds before the
// end, and climbs back over just before time runs out - so it always looks
// like it almost lost. Harder materials swing wider and finish closer to the
// bar. Below diamond the CPU always wins. On diamond the CPU's result is the
// opposite of the player's, which isn't known until the buzzer: it settles
// exactly on the bar, then `resolve_diamond` tips it just over (player lost)
// or just under (player won), and the eased gauge shows that as the strike
// lands.
struct CpuRun {
    material: Material,
    // Swing size either side of the bar.
    amplitude: f64,
    // Where it lands at the end of the test. On diamond this is exactly the
    // bar (1.0) until `resolve_diamond` is called.
    final_ratio: f64,
    swing_hz: f64,
    // Chosen so the swing bottoms out exactly at CPU_DIP_SECS.
    swing_phase: f64,
}

// Seconds for the CPU gauge to charge up at the start, like the player's.
const CPU_RAMP_SECS: f64 = 3.0;
// When the last dip under the bar bottoms out; from here it settles onto its
// final value by the end of the test.
const CPU_DIP_SECS: f64 = 27.0;
const TEST_SECS: f64 = 30.0;
// On diamond, how far past the bar (either way) the CPU finishes.
const DIAMOND_MARGIN: std::ops::Range<f64> = 0.03..0.06;

impl CpuRun {
    fn new(material: Material) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        // (swing amplitude, winning finish range) - wider swings and thinner
        // winning margins as the materials get harder. Diamond's finish is
        // decided later by the player's result.
        let (amplitude, final_ratio) = match material {
            Material::Wood => (0.15, rng.gen_range(1.10..1.18)),
            Material::Stone => (0.18, rng.gen_range(1.07..1.12)),
            Material::Steel => (0.22, rng.gen_range(1.05..1.09)),
            Material::Ruby => (0.26, rng.gen_range(1.03..1.06)),
            Material::Diamond => (0.30, 1.0),
        };
        let swing_hz = rng.gen_range(0.18..0.28);
        let swing_phase = -std::f64::consts::FRAC_PI_2 - std::f64::consts::TAU * swing_hz * CPU_DIP_SECS;
        CpuRun { material, amplitude, final_ratio, swing_hz, swing_phase }
    }

    // On diamond, sets the CPU's finish to the opposite of the player's
    // result. No-op on other materials, where the CPU always wins.
    fn resolve_diamond(&mut self, player_passed: bool) {
        use rand::Rng;
        if self.material != Material::Diamond {
            return;
        }
        let margin = rand::thread_rng().gen_range(DIAMOND_MARGIN);
        self.final_ratio = if player_passed { 1.0 - margin } else { 1.0 + margin };
    }

    // Whether the CPU's final speed clears the bar (breaks its slab). Only
    // meaningful once the round is over (and, on diamond, resolved).
    fn passes(&self) -> bool {
        self.final_ratio > 1.0
    }

    // Speed as a multiple of the target, `t` seconds into the test.
    fn ratio_at(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, TEST_SECS);
        let ramp = smoothstep(t / CPU_RAMP_SECS);
        let swing = 1.0 + self.amplitude * (std::f64::consts::TAU * self.swing_hz * t + self.swing_phase).sin();
        let settle = smoothstep((t - CPU_DIP_SECS) / (TEST_SECS - CPU_DIP_SECS));
        ramp * (swing * (1.0 - settle) + self.final_ratio * settle)
    }
}

// 0 below 0, 1 above 1, and an eased S-curve in between.
fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

// Draws `tex` at its native pixel size (never resized) with its top-left at
// `min`, in screen coordinates.
fn draw_sprite(painter: &egui::Painter, tex: &egui::TextureHandle, min: egui::Pos2, tint: egui::Color32) {
    painter.image(
        tex.id(),
        egui::Rect::from_min_size(min, tex.size_vec2()),
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        tint,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_hits_the_bar_at_target_speed_and_stays_in_the_gauge() {
        for m in Material::ALL {
            assert_eq!(fill_fraction(1.0, m.bar_fraction()), m.bar_fraction());
            assert_eq!(fill_fraction(100.0, m.bar_fraction()), 1.0);
            assert_eq!(fill_fraction(-1.0, m.bar_fraction()), 0.0);
        }
    }

    #[test]
    fn ending_strikes_then_breaks_then_celebrates() {
        let impact = Pose::Strike.secs_to_last_frame();
        let victory = impact + VICTORY_DELAY_SECS;
        let at = |t: f64| ending_at(100.0, true, 100.0 + t);
        assert_eq!(at(0.0), Ending { pose: Pose::Strike, pose_started: 100.0, broken: false, next_change: Some(100.0 + impact) });
        assert_eq!(at(impact), Ending { pose: Pose::Strike, pose_started: 100.0, broken: true, next_change: Some(100.0 + victory) });
        assert_eq!(at(victory), Ending { pose: Pose::Victory, pose_started: 100.0 + victory, broken: true, next_change: None });
        assert_eq!(at(60.0).pose, Pose::Victory);
    }

    #[test]
    fn ending_holds_the_strike_when_too_slow() {
        for t in [0.0, 1.0, 60.0] {
            assert_eq!(
                ending_at(100.0, false, 100.0 + t),
                Ending { pose: Pose::Strike, pose_started: 100.0, broken: false, next_change: None }
            );
        }
    }

    #[test]
    fn cpu_swings_over_and_under_the_bar_and_nearly_loses() {
        let crossings = |cpu: &CpuRun| {
            let samples: Vec<f64> = (400..=2700).map(|i| cpu.ratio_at(i as f64 / 100.0) - 1.0).collect();
            samples.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count()
        };
        for _ in 0..200 {
            for m in Material::ALL {
                let cpu = CpuRun::new(m);
                assert!(crossings(&cpu) >= 6, "{m:?} CPU should keep crossing the bar");
                assert!(cpu.ratio_at(CPU_DIP_SECS) < 1.0, "{m:?} CPU should dip under right before the end");
            }
        }
        // Harder materials swing wider.
        let amps: Vec<f64> = Material::ALL.map(|m| CpuRun::new(m).amplitude).to_vec();
        assert!(amps.windows(2).all(|w| w[1] > w[0]));
    }

    #[test]
    fn cpu_always_wins_below_diamond() {
        for _ in 0..200 {
            for m in [Material::Wood, Material::Stone, Material::Steel, Material::Ruby] {
                for player_passed in [true, false] {
                    let mut cpu = CpuRun::new(m);
                    cpu.resolve_diamond(player_passed);
                    assert!(cpu.ratio_at(TEST_SECS) > 1.0, "{m:?}");
                    assert!(cpu.passes(), "{m:?}");
                }
            }
        }
    }

    #[test]
    fn diamond_cpu_gets_the_opposite_of_the_player() {
        for _ in 0..200 {
            let cpu = CpuRun::new(Material::Diamond);
            assert_eq!(cpu.ratio_at(TEST_SECS), 1.0, "undecided diamond CPU sits on the bar at the buzzer");
            for player_passed in [true, false] {
                let mut cpu = CpuRun::new(Material::Diamond);
                cpu.resolve_diamond(player_passed);
                assert_eq!(cpu.passes(), !player_passed);
                assert_eq!(cpu.ratio_at(TEST_SECS) > 1.0, !player_passed);
            }
        }
    }
}
