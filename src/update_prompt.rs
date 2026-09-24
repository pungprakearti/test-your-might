// The in-game side of self-updating (see update.rs): checks for a newer
// release in the background at startup, and when there is one, offers it on
// the character select screen. ENTER downloads, verifies, and installs it,
// then the game restarts into the new version; ESC dismisses it until the
// next launch. While the panel is up it takes the keyboard, so ENTER doesn't
// also pick a fighter.

use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::update::{self, Installed, Release, Version};

const PANEL_WIDTH: f32 = 380.0;
const PANEL_PAD: egui::Vec2 = egui::vec2(16.0, 12.0);
const TITLE_FONT: f32 = 16.0;
const TEXT_FONT: f32 = 13.0;
const LINE_GAP: f32 = 6.0;
const DIM_ALPHA: u8 = 150;
const PANEL_ALPHA: u8 = 225;
const TITLE_COLOR: egui::Color32 = egui::Color32::from_rgb(240, 200, 40);
const TEXT_COLOR: egui::Color32 = egui::Color32::from_gray(200);
const KEYS_COLOR: egui::Color32 = egui::Color32::from_rgb(120, 220, 140);
const ERROR_COLOR: egui::Color32 = egui::Color32::from_rgb(235, 70, 70);
const BAR_HEIGHT: f32 = 8.0;
const BAR_BG: egui::Color32 = egui::Color32::from_gray(60);

enum State {
    // Checking, up to date, dismissed, or the check failed (logged only -
    // being offline shouldn't nag).
    Idle,
    Offer(Release),
    Installing { version: Version, progress: f32 },
    Failed(String),
    Installed(Installed),
    // The new version has been started and this one is closing (can take a
    // frame or two, which must not start it again).
    Restarting,
}

pub struct UpdatePrompt {
    state: Arc<Mutex<State>>,
}

impl UpdatePrompt {
    pub fn start(ctx: &egui::Context) -> Self {
        let state = Arc::new(Mutex::new(State::Idle));
        if update::startup_check_enabled() {
            let (state, ctx) = (state.clone(), ctx.clone());
            std::thread::spawn(move || match update::check() {
                Ok(Some(release)) => {
                    *state.lock().unwrap() = State::Offer(release);
                    ctx.request_repaint();
                }
                Ok(None) => {}
                Err(e) => eprintln!("update check: {e}"),
            });
        }
        Self { state }
    }

    // Whether the panel is up (so the screen underneath shouldn't get input).
    pub fn active(&self) -> bool {
        !matches!(*self.state.lock().unwrap(), State::Idle)
    }

    // Returns true when the game should quit because the new version has
    // been started.
    pub fn handle_input(&mut self, ctx: &egui::Context) -> bool {
        let (enter, esc) = ctx.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
        let mut state = self.state.lock().unwrap();
        match &*state {
            State::Offer(release) if enter => {
                let release = release.clone();
                *state = State::Installing { version: release.version, progress: 0.0 };
                let (shared, ctx) = (self.state.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let progress = |p: f32| {
                        if let State::Installing { progress, .. } = &mut *shared.lock().unwrap() {
                            *progress = p;
                        }
                        ctx.request_repaint();
                    };
                    let result = update::install(&release, &progress);
                    *shared.lock().unwrap() = match result {
                        Ok(installed) => State::Installed(installed),
                        Err(e) => State::Failed(e),
                    };
                    ctx.request_repaint();
                });
            }
            State::Offer(_) | State::Failed(_) if esc || enter => *state = State::Idle,
            State::Installed(installed) => match update::relaunch(installed) {
                Ok(()) => {
                    *state = State::Restarting;
                    return true;
                }
                Err(e) => {
                    *state = State::Failed(format!("The update is installed. {e} Start it again yourself."));
                }
            },
            _ => {}
        }
        false
    }

    pub fn draw(&self, ui: &egui::Ui, screen_rect: egui::Rect) {
        let state = self.state.lock().unwrap();
        let current = Version::current();
        let mut lines: Vec<(String, f32, egui::Color32)> = Vec::new();
        let mut bar = None;
        match &*state {
            State::Idle => return,
            State::Offer(release) => {
                lines.push(("UPDATE AVAILABLE".into(), TITLE_FONT, TITLE_COLOR));
                lines.push((format!("Version {}  (you have {current})", release.version), TEXT_FONT, TEXT_COLOR));
                lines.push(("ENTER update   ESC not now".into(), TEXT_FONT, KEYS_COLOR));
            }
            State::Installing { version, progress } => {
                lines.push((format!("UPDATING TO {version}"), TITLE_FONT, TITLE_COLOR));
                let what = if *progress < 1.0 {
                    format!("Downloading {:.0}%", progress * 100.0)
                } else {
                    "Installing...".into()
                };
                lines.push((what, TEXT_FONT, TEXT_COLOR));
                bar = Some(*progress);
            }
            State::Failed(message) => {
                lines.push(("UPDATE FAILED".into(), TITLE_FONT, ERROR_COLOR));
                lines.push((message.clone(), TEXT_FONT, TEXT_COLOR));
                lines.push(("ESC continue".into(), TEXT_FONT, KEYS_COLOR));
            }
            State::Installed(_) | State::Restarting => {
                lines.push(("RESTARTING...".into(), TITLE_FONT, TITLE_COLOR));
            }
        }

        let painter = ui.painter();
        let text_width = PANEL_WIDTH - PANEL_PAD.x * 2.0;
        let galleys: Vec<_> = lines
            .into_iter()
            .map(|(text, size, color)| {
                let mut job = egui::text::LayoutJob::simple(text, egui::FontId::monospace(size), color, text_width);
                job.halign = egui::Align::Center;
                painter.layout_job(job)
            })
            .collect();
        let mut height = galleys.iter().map(|g| g.size().y).sum::<f32>()
            + LINE_GAP * (galleys.len() as f32 - 1.0)
            + PANEL_PAD.y * 2.0;
        if bar.is_some() {
            height += LINE_GAP + BAR_HEIGHT;
        }

        painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(DIM_ALPHA));
        let panel = egui::Rect::from_center_size(screen_rect.center(), egui::vec2(PANEL_WIDTH, height));
        painter.rect_filled(panel, 6.0, egui::Color32::from_black_alpha(PANEL_ALPHA));
        painter.rect_stroke(panel, 6.0, egui::Stroke::new(1.0f32, TITLE_COLOR.gamma_multiply(0.6)));
        let mut y = panel.min.y + PANEL_PAD.y;
        for galley in galleys {
            // Centered lines are laid out around x = 0.
            let x = panel.center().x - galley.rect.center().x;
            let h = galley.size().y;
            painter.galley(egui::pos2(x, y), galley, TEXT_COLOR);
            y += h + LINE_GAP;
        }
        if let Some(progress) = bar {
            let track = egui::Rect::from_min_size(egui::pos2(panel.min.x + PANEL_PAD.x, y), egui::vec2(text_width, BAR_HEIGHT));
            painter.rect_filled(track, 3.0, BAR_BG);
            let fill = egui::Rect::from_min_size(track.min, egui::vec2(text_width * progress, BAR_HEIGHT));
            painter.rect_filled(fill, 3.0, KEYS_COLOR);
        }
    }
}
