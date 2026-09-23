// Typing test: monkeytype-style fixed word stream, per-character correctness
// tracking, and the 3-line scrolling word view. Drawn by the Test Your Might
// screen onto the 436x83 concrete floor strip, so the layout is compact.

use eframe::egui;
use std::time::{Duration, Instant};

const TEST_DURATION: Duration = Duration::from_secs(30);
// Window for the "current speed" reading that drives the gauge.
const LIVE_WINDOW_SECS: f64 = 3.0;

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
    // Unique per test, so per-letter animation state from a previous test
    // (egui keys it by id) doesn't carry over and suppress the pop-in.
    id: u64,
    words: Vec<TypedWord>,
    current_word: usize,
    started_at: Option<Instant>,
    correct_chars: usize,
    incorrect_chars: usize,
    total_keystrokes: usize,
    correct_keystrokes: usize,
    finished: bool,
    final_elapsed: Option<Duration>,
    // (seconds since start, correct_chars) after every change, for the
    // rolling live-speed reading.
    samples: Vec<(f64, usize)>,
}

impl TestState {
    fn new() -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            words: Self::gen_words(60),
            current_word: 0,
            started_at: None,
            correct_chars: 0,
            incorrect_chars: 0,
            total_keystrokes: 0,
            correct_keystrokes: 0,
            finished: false,
            final_elapsed: None,
            samples: Vec::new(),
        }
    }

    fn gen_words(n: usize) -> Vec<TypedWord> {
        Self::gen_words_after(n, None)
    }

    fn gen_words_after(n: usize, mut prev: Option<&str>) -> Vec<TypedWord> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        (0..n)
            .map(|_| {
                let mut w: &str = WORDS.choose(&mut rng).unwrap();
                while Some(w) == prev {
                    w = WORDS.choose(&mut rng).unwrap();
                }
                prev = Some(w);
                let w = w.to_string();
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
        self.apply_char(c);
        self.samples.push((self.elapsed().as_secs_f64(), self.correct_chars));
    }

    fn apply_char(&mut self, c: char) {
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
        self.samples.push((self.elapsed().as_secs_f64(), self.correct_chars));
    }

    fn commit_word(&mut self) {
        // Only advance if something was typed, monkeytype-style (no leading spaces).
        if self.words[self.current_word].typed.is_empty() {
            return;
        }
        self.current_word += 1;
        // Keep a healthy lookahead buffer past the current word at all times,
        // instead of only topping up once the buffer is fully exhausted -
        // otherwise the visible lines ahead of the caret can go blank while
        // still typing the last buffered word, before the top-up fires.
        const LOOKAHEAD: usize = 40;
        if self.words.len() - self.current_word < LOOKAHEAD {
            let prev = self.words.last().map(|w| w.target.as_str());
            self.words.extend(Self::gen_words_after(LOOKAHEAD, prev));
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

    // WPM over just the last LIVE_WINDOW_SECS (always divided by the full
    // window, so it charges up from 0 over the first few seconds instead of
    // spiking on the first word).
    fn rolling_wpm(&self) -> f64 {
        let now = self.final_elapsed.unwrap_or_else(|| self.elapsed()).as_secs_f64();
        let then = now - LIVE_WINDOW_SECS;
        let chars_then = self.samples.iter().rev().find(|(t, _)| *t <= then).map(|&(_, c)| c).unwrap_or(0);
        let chars = self.correct_chars.saturating_sub(chars_then);
        (chars as f64 / 5.0) / (LIVE_WINDOW_SECS / 60.0)
    }

    // How far through the test we are, 0..=1.
    fn progress(&self) -> f64 {
        (self.final_elapsed.unwrap_or_else(|| self.elapsed()).as_secs_f64() / TEST_DURATION.as_secs_f64()).min(1.0)
    }

    // The speed shown on the gauge: early on it follows the rolling current
    // speed, and it blends steadily toward the overall test WPM as the test
    // goes on, landing exactly on the final WPM (the pass/fail number) at the
    // end.
    fn live_wpm(&self) -> f64 {
        let t = self.progress();
        self.rolling_wpm() * (1.0 - t) + self.wpm() * t
    }

    fn accuracy(&self) -> f64 {
        if self.total_keystrokes == 0 {
            return 100.0;
        }
        (self.correct_keystrokes as f64 / self.total_keystrokes as f64) * 100.0
    }
}

pub struct TypingScreen {
    test: TestState,
    caret_pos: Option<egui::Pos2>,
}

impl TypingScreen {
    pub fn new() -> Self {
        Self { test: TestState::new(), caret_pos: None }
    }

    // Feeds text straight into the test as if typed (dev hook).
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            self.test.on_char(c);
        }
    }

    // Correctly types the first `n` words of the test, each followed by a
    // space (dev hook).
    pub fn type_words(&mut self, n: usize) {
        for _ in 0..n {
            let word = self.test.words[self.test.current_word].target.clone();
            self.type_text(&word);
            self.type_text(" ");
        }
    }

    pub fn handle_input(&mut self, ctx: &egui::Context) {
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
        }
    }

    pub fn started(&self) -> bool {
        self.test.started_at.is_some()
    }

    pub fn finished(&self) -> bool {
        self.test.finished
    }

    // Seconds since the first keypress (frozen at the test length once done).
    pub fn elapsed_secs(&self) -> f64 {
        self.test.final_elapsed.unwrap_or_else(|| self.test.elapsed()).as_secs_f64()
    }

    pub fn wpm(&self) -> f64 {
        self.test.wpm()
    }

    pub fn accuracy(&self) -> f64 {
        self.test.accuracy()
    }

    pub fn live_wpm(&self) -> f64 {
        self.test.live_wpm()
    }

    // `status` is shown right-aligned in the header row.
    pub fn draw(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, rect: egui::Rect, status: &str) {
        let dt = ctx.input(|i| i.stable_dt);
        draw_screen(ui, &self.test, rect, &mut self.caret_pos, dt, status);
    }
}

const VISIBLE_LINES: usize = 3;
const LINE_GAP: f32 = 2.0;
const PAD_X: f32 = 8.0;
const HEADER_FONT: f32 = 12.0;
const HEADER_GAP: f32 = 2.0;
const WORD_FONT: f32 = 14.0;
// Dark wash over the concrete so the gray untyped words stay readable while
// the floor texture still shows through.
const BACKDROP_ALPHA: u8 = 150;
const POP_IN_SECS: f32 = 0.12;
const POP_RISE: f32 = 5.0;

fn draw_screen(
    ui: &mut egui::Ui,
    test: &TestState,
    rect: egui::Rect,
    caret_pos: &mut Option<egui::Pos2>,
    dt: f32,
    status: &str,
) {
    let ctx = ui.ctx().clone();
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(BACKDROP_ALPHA));

    // Header row + 3 word lines, centered vertically in the strip.
    let header_font = egui::FontId::monospace(HEADER_FONT);
    let header_h = ui.fonts(|f| f.row_height(&header_font));
    let font = egui::FontId::monospace(WORD_FONT);
    let line_h = ui.fonts(|f| f.row_height(&font));
    let content_h =
        header_h + HEADER_GAP + VISIBLE_LINES as f32 * line_h + (VISIBLE_LINES - 1) as f32 * LINE_GAP;
    let top = (rect.center().y - content_h / 2.0).round();
    let inner = egui::Rect::from_x_y_ranges(rect.min.x + PAD_X..=rect.max.x - PAD_X, top..=top + content_h);

    // Header: timer / results
    let header_rect = egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), header_h));
    if test.finished {
        painter.text(
            header_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            format!("WPM {:.0}   ACC {:.0}%", test.wpm(), test.accuracy()),
            header_font.clone(),
            egui::Color32::from_rgb(120, 220, 140),
        );
    } else {
        let secs_left = test.remaining().as_secs();
        painter.text(
            header_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            format!("{:>2}s   wpm {:.0}", secs_left, test.wpm()),
            header_font.clone(),
            egui::Color32::from_rgb(200, 200, 210),
        );
    }

    painter.text(
        header_rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        status,
        header_font.clone(),
        egui::Color32::from_rgb(240, 200, 40),
    );

    // Word area
    let words_top = inner.min.y + header_h + HEADER_GAP;
    let words_rect = egui::Rect::from_min_max(egui::pos2(inner.min.x, words_top), inner.max);

    let space_w = ui.fonts(|f| f.glyph_width(&font, ' '));

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
            let anim_id = egui::Id::new(("char-pop", test.id, p.wi, p.ci));
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
