// Test Your Might - typing test scaffold.
// Window is fixed at 500x700 to match mk-cabinet.png. The typing test renders
// only inside the "screen" rect of that image: (32,151) -> (467,441).

use eframe::egui;
use std::time::{Duration, Instant};

const WINDOW_W: f32 = 500.0;
const WINDOW_H: f32 = 700.0;

// Screen rect measured from mk-cabinet.png (flood-filled bounding box of the
// blue-gray panel).
const SCREEN_MIN: egui::Pos2 = egui::pos2(32.0, 151.0);
const SCREEN_MAX: egui::Pos2 = egui::pos2(467.0, 441.0);

const TEST_DURATION: Duration = Duration::from_secs(30);

const WORDS: &[&str] = &[
    "the", "of", "and", "to", "in", "is", "you", "that", "it", "he", "was", "for", "on", "are",
    "with", "as", "his", "they", "at", "be", "this", "have", "from", "or", "one", "had", "word",
    "but", "not", "what", "all", "were", "when", "your", "can", "said", "there", "use", "each",
    "which", "she", "how", "their", "will", "up", "other", "about", "out", "many", "then",
    "them", "time", "look", "way", "could", "people", "than", "first", "water", "call", "who",
    "may", "down", "side", "been", "now", "find", "any", "new", "work", "part", "take", "get",
    "place", "made", "live", "where", "after", "back", "little", "only", "round", "man", "year",
    "came", "show", "every", "good", "give", "under", "name", "very", "through", "just", "form",
    "sentence", "great", "think", "say", "help", "low", "line", "differ", "turn", "cause",
];

#[derive(Clone, Copy, PartialEq)]
enum CharState {
    Untyped,
    Correct,
    Incorrect,
}

struct TypedWord {
    target: String,
    states: Vec<CharState>,
    typed: String,
}

struct TestState {
    words: Vec<TypedWord>,
    current_word: usize,
    started_at: Option<Instant>,
    correct_chars: usize,
    incorrect_chars: usize,
    total_keystrokes: usize,
    correct_keystrokes: usize,
    finished: bool,
    final_elapsed: Option<Duration>,
}

impl TestState {
    fn new() -> Self {
        Self {
            words: Self::gen_words(60),
            current_word: 0,
            started_at: None,
            correct_chars: 0,
            incorrect_chars: 0,
            total_keystrokes: 0,
            correct_keystrokes: 0,
            finished: false,
            final_elapsed: None,
        }
    }

    fn gen_words(n: usize) -> Vec<TypedWord> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        (0..n)
            .map(|_| {
                let w = WORDS.choose(&mut rng).unwrap().to_string();
                let states = vec![CharState::Untyped; w.len()];
                TypedWord {
                    target: w,
                    states,
                    typed: String::new(),
                }
            })
            .collect()
    }

    fn elapsed(&self) -> Duration {
        self.started_at.map(|s| s.elapsed()).unwrap_or_default()
    }

    fn remaining(&self) -> Duration {
        TEST_DURATION.saturating_sub(self.elapsed())
    }

    fn on_char(&mut self, c: char) {
        if self.finished {
            return;
        }
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        if c == ' ' {
            self.commit_word();
            return;
        }
        if c.is_control() {
            return;
        }
        let word = &mut self.words[self.current_word];
        let idx = word.typed.chars().count();
        word.typed.push(c);
        self.total_keystrokes += 1;
        if let Some(expected) = word.target.chars().nth(idx) {
            if expected == c {
                self.correct_chars += 1;
                self.correct_keystrokes += 1;
                if idx < word.states.len() {
                    word.states[idx] = CharState::Correct;
                }
            } else {
                self.incorrect_chars += 1;
                if idx < word.states.len() {
                    word.states[idx] = CharState::Incorrect;
                } else {
                    word.states.push(CharState::Incorrect);
                }
            }
        } else {
            // extra char beyond target length
            self.incorrect_chars += 1;
            word.states.push(CharState::Incorrect);
        }
    }

    fn on_backspace(&mut self) {
        if self.finished {
            return;
        }
        // If the current word is empty, back up into the previous word(s)
        // instead of stopping dead, so a run of mistakes further back can be
        // fixed without retyping everything since.
        while self.words[self.current_word].typed.is_empty() && self.current_word > 0 {
            self.current_word -= 1;
        }
        let word = &mut self.words[self.current_word];
        if let Some(c) = word.typed.pop() {
            let idx = word.typed.chars().count();
            if idx < word.states.len() {
                match word.states[idx] {
                    CharState::Correct => self.correct_chars = self.correct_chars.saturating_sub(1),
                    CharState::Incorrect => self.incorrect_chars = self.incorrect_chars.saturating_sub(1),
                    CharState::Untyped => {}
                }
                word.states[idx] = CharState::Untyped;
            } else {
                word.states.pop();
                self.incorrect_chars = self.incorrect_chars.saturating_sub(1);
            }
            let _ = c;
        }
    }

    fn commit_word(&mut self) {
        // Only advance if something was typed, monkeytype-style (no leading spaces).
        if self.words[self.current_word].typed.is_empty() {
            return;
        }
        self.current_word += 1;
        if self.current_word >= self.words.len() {
            self.words.extend(Self::gen_words(40));
        }
    }

    fn tick(&mut self) {
        if !self.finished && self.started_at.is_some() && self.remaining().is_zero() {
            self.finished = true;
            self.final_elapsed = Some(TEST_DURATION);
        }
    }

    fn wpm(&self) -> f64 {
        let mins = self
            .final_elapsed
            .unwrap_or_else(|| self.elapsed())
            .as_secs_f64()
            / 60.0;
        if mins <= 0.0 {
            return 0.0;
        }
        (self.correct_chars as f64 / 5.0) / mins
    }

    fn accuracy(&self) -> f64 {
        if self.total_keystrokes == 0 {
            return 100.0;
        }
        (self.correct_keystrokes as f64 / self.total_keystrokes as f64) * 100.0
    }
}

struct App {
    bg_texture: Option<egui::TextureHandle>,
    test: TestState,
    caret_pos: Option<egui::Pos2>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let bg_texture = load_bg_texture(&cc.egui_ctx);
        Self {
            bg_texture,
            test: TestState::new(),
            caret_pos: None,
        }
    }
}

fn load_bg_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let bytes = include_bytes!("../mk-cabinet.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let pixels = img.into_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
    Some(ctx.load_texture("cabinet-bg", color_image, egui::TextureOptions::LINEAR))
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.test.tick();
        if !self.test.finished {
            ctx.input(|i| {
                for ev in &i.events {
                    match ev {
                        egui::Event::Text(t) => {
                            for c in t.chars() {
                                self.test.on_char(c);
                            }
                        }
                        egui::Event::Key {
                            key: egui::Key::Backspace,
                            pressed: true,
                            ..
                        } => self.test.on_backspace(),
                        _ => {}
                    }
                }
            });
        } else {
            let restart = ctx.input(|i| i.key_pressed(egui::Key::R));
            if restart {
                self.test = TestState::new();
                self.caret_pos = None;
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
                let dt = ctx.input(|i| i.stable_dt);
                draw_screen(&mut screen_ui, &self.test, screen_rect, &mut self.caret_pos, dt);

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

const VISIBLE_LINES: usize = 3;
const LINE_GAP: f32 = 6.0;
const POP_IN_SECS: f32 = 0.12;
const POP_RISE: f32 = 5.0;

fn draw_screen(
    ui: &mut egui::Ui,
    test: &TestState,
    rect: egui::Rect,
    caret_pos: &mut Option<egui::Pos2>,
    dt: f32,
) {
    let ctx = ui.ctx().clone();
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 22, 30));

    let pad = 10.0;
    let inner = rect.shrink(pad);

    // Header: timer / results
    let header_h = 28.0;
    let header_rect = egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), header_h));
    if test.finished {
        painter.text(
            header_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            format!("WPM {:.0}   ACC {:.0}%   (press R)", test.wpm(), test.accuracy()),
            egui::FontId::monospace(16.0),
            egui::Color32::from_rgb(120, 220, 140),
        );
    } else {
        let secs_left = test.remaining().as_secs();
        painter.text(
            header_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            format!("{:>2}s   wpm {:.0}", secs_left, test.wpm()),
            egui::FontId::monospace(16.0),
            egui::Color32::from_rgb(200, 200, 210),
        );
    }

    // Word area
    let words_top = inner.min.y + header_h + 6.0;
    let words_rect = egui::Rect::from_min_max(egui::pos2(inner.min.x, words_top), inner.max);

    let font = egui::FontId::monospace(15.0);
    let space_w = ui.fonts(|f| f.glyph_width(&font, ' '));
    let line_h = ui.fonts(|f| f.row_height(&font));

    // --- Layout pass: figure out which line each word falls on, without
    // drawing anything yet, so we know which lines are visible and where the
    // caret should end up before we start painting.
    struct PlacedChar {
        wi: usize,
        ci: usize,
        c: char,
        line: usize,
        x: f32,
        w: f32,
    }
    let mut placed: Vec<PlacedChar> = Vec::new();
    let mut line = 0usize;
    let mut x = 0.0f32;
    let max_w = words_rect.width();
    let mut current_word_end: Option<(usize, f32)> = None; // (line, x)

    for (wi, word) in test.words.iter().enumerate() {
        let mut word_w = 0.0;
        for c in word.target.chars() {
            word_w += ui.fonts(|f| f.glyph_width(&font, c));
        }
        if x + word_w > max_w && x > 0.0 {
            line += 1;
            x = 0.0;
        }
        for (ci, c) in word.target.chars().enumerate() {
            let cw = ui.fonts(|f| f.glyph_width(&font, c));
            placed.push(PlacedChar { wi, ci, c, line, x, w: cw });
            x += cw;
        }
        if wi == test.current_word {
            current_word_end = Some((line, x));
        }
        x += space_w;
        // Stop building layout well past what could ever be visible.
        if line > VISIBLE_LINES + 20 {
            break;
        }
    }

    let active_line = placed
        .iter()
        .find(|p| p.wi == test.current_word)
        .map(|p| p.line)
        .or_else(|| current_word_end.map(|(l, _)| l))
        .unwrap_or(0);
    let start_line = active_line.saturating_sub(1);
    let end_line = start_line + VISIBLE_LINES;

    // Caret target: end of the current word's typed text, or its start.
    let typed_count = test.words[test.current_word].typed.chars().count();
    let caret_target_xy = if typed_count == 0 {
        placed
            .iter()
            .find(|p| p.wi == test.current_word)
            .map(|p| (p.line, p.x))
            .or(current_word_end)
            .unwrap_or((active_line, 0.0))
    } else {
        placed
            .iter()
            .filter(|p| p.wi == test.current_word && p.ci == typed_count - 1)
            .map(|p| (p.line, p.x + p.w))
            .next()
            .unwrap_or((active_line, 0.0))
    };
    let caret_target = egui::pos2(
        words_rect.min.x + caret_target_xy.1,
        words_rect.min.y + (caret_target_xy.0 as f32 - start_line as f32) * (line_h + LINE_GAP),
    );
    let caret_now = match caret_pos {
        Some(p) => {
            let speed = 1.0 - (-dt * 22.0).exp();
            *p = p.lerp(caret_target, speed.clamp(0.0, 1.0));
            *p
        }
        None => {
            *caret_pos = Some(caret_target);
            caret_target
        }
    };

    // --- Paint pass: only characters on the visible lines get drawn.
    let painter = ui.painter();
    for p in &placed {
        if p.line < start_line || p.line >= end_line {
            continue;
        }
        let word = &test.words[p.wi];
        let is_current = p.wi == test.current_word;
        let typed_here = p.ci < word.typed.chars().count();

        let color = if typed_here {
            match word.states.get(p.ci) {
                Some(CharState::Correct) => egui::Color32::from_rgb(240, 240, 245),
                Some(CharState::Incorrect) => egui::Color32::from_rgb(235, 70, 70),
                _ => egui::Color32::from_gray(150),
            }
        } else if is_current {
            egui::Color32::from_gray(155)
        } else {
            egui::Color32::from_gray(95)
        };

        let pos = egui::pos2(
            words_rect.min.x + p.x,
            words_rect.min.y + (p.line as f32 - start_line as f32) * (line_h + LINE_GAP),
        );

        if typed_here {
            // Pop each newly-typed letter in: it eases up from slightly below
            // and fades in, instead of snapping into place.
            let anim_id = egui::Id::new(("char-pop", p.wi, p.ci));
            let t = ctx.animate_value_with_time(anim_id, 1.0, POP_IN_SECS);
            let eased = 1.0 - (1.0 - t) * (1.0 - t);
            let offset_y = (1.0 - eased) * POP_RISE;
            let alpha = (0.35 + 0.65 * eased).clamp(0.0, 1.0);
            let c = color.gamma_multiply(alpha);
            painter.text(
                pos + egui::vec2(0.0, offset_y),
                egui::Align2::LEFT_TOP,
                p.c,
                font.clone(),
                c,
            );
        } else {
            painter.text(pos, egui::Align2::LEFT_TOP, p.c, font.clone(), color);
        }
    }

    if !test.finished {
        painter.line_segment(
            [caret_now, caret_now + egui::vec2(0.0, line_h)],
            egui::Stroke::new(1.5f32, egui::Color32::WHITE),
        );
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
