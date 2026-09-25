// Release builds are a windowed app on Windows: no console window behind the
// game. Debug builds keep the console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Test Your Might - arcade cabinet app scaffold.
// Everything is laid out in 500x700 design points to match mk-cabinet.png;
// the window itself is sized from the monitor's height (see fit_window). Each
// screen (character select, then Test Your Might with the typing test on its
// floor) renders only inside the cabinet's "screen" rect of that image:
// (32,151) -> (467,441) inclusive, 436x291. See char_select.rs,
// test_your_might.rs, and typing_test.rs for the individual screens.
//
// The same game runs on the web (wasm32, see docs/web.md): in a browser tab
// the cabinet is zoomed to fit the page instead of sizing a window, and
// there's no Close button, window dragging, or self-updating.

// A warning for the log: stderr on the desktop, the browser's console on the
// web (where stderr goes nowhere).
macro_rules! warn {
    ($($arg:tt)*) => {{
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!($($arg)*);
        #[cfg(target_arch = "wasm32")]
        web_sys::console::warn_1(&format!($($arg)*).into());
    }};
}

mod audio;
mod char_select;
mod dev;
mod fighter;
mod progress;
mod test_your_might;
mod typing_test;
#[cfg(not(target_arch = "wasm32"))]
mod update;
#[cfg(not(target_arch = "wasm32"))]
mod update_prompt;

use eframe::egui;
use std::time::Duration;

use audio::{Audio, Sound};
use char_select::CharSelectScreen;
use test_your_might::TestYourMightScreen;
#[cfg(not(target_arch = "wasm32"))]
use update_prompt::UpdatePrompt;

pub const REPO_URL: &str = "https://github.com/pungprakearti/test-your-might";

const WINDOW_W: f32 = 500.0;
const WINDOW_H: f32 = 700.0;
const WINDOW_SIZE: egui::Vec2 = egui::vec2(WINDOW_W, WINDOW_H);
// Window height as a fraction of the monitor's height (see fit_window).
#[cfg(not(target_arch = "wasm32"))]
const HEIGHT_FRACTION: f32 = 0.5;
// Room left above and beside the cabinet on a web page, in CSS pixels.
#[cfg(target_arch = "wasm32")]
const PAGE_MARGIN: f32 = 16.0;
// How long the window must sit still on a monitor before it's resized for it.
#[cfg(not(target_arch = "wasm32"))]
const SETTLE_SECS: f64 = 0.4;
// How often to re-ask for the right window size while it's still wrong.
#[cfg(not(target_arch = "wasm32"))]
const RESIZE_RETRY_SECS: f64 = 1.0;

// Screen rect measured from mk-cabinet.png (flood-filled bounding box of the
// blue-gray panel). The panel's last pixel column/row is 467/441, so the
// exclusive max is one past that - 436x291, matching the screen art exactly so
// it's drawn 1:1 rather than resampled.
const SCREEN_MIN: egui::Pos2 = egui::pos2(32.0, 151.0);
const SCREEN_MAX: egui::Pos2 = egui::pos2(468.0, 442.0);

// Clickable icons that open a web page, in a row on the bottom right of the
// cabinet's lower panel (x 17-482, from y 597 in mk-cabinet.png), mirroring
// the coin plate on the left: the plate starts 30px in from the panel's left
// edge at y 625, so the row's top is at y 625 and its right edge 30px in
// from the panel's right edge. Each icon is centered, at its own aspect
// ratio, in a LINK_ICON_SIZE box; the boxes are 70px apart center to center,
// like the plate's two coin slots (x 63-91 and 133-161).
//
// Each PNG is its SVG in assets/ rendered as a white silhouette at 4x (200px)
// plus an 8px transparent margin, so edges aren't clipped when drawn:
// `cargo run --example render_svg -- <svg> <png> 216 8`, which also prints
// the drawing's aspect ratio. Tinted when drawn.
struct LinkIcon {
    id: &'static str,
    png: &'static [u8],
    // Left edge of the icon's box.
    left: f32,
    // Width / height of the drawing.
    aspect: f32,
    url: &'static str,
}

const LINK_ICONS: [LinkIcon; 2] = [
    LinkIcon {
        id: "hockey-puck",
        png: include_bytes!("../assets/hockey-puck.png"),
        left: 333.0,
        aspect: 1.2858,
        url: "https://www.biscuitsinthebasket.com",
    },
    LinkIcon {
        id: "github-logo",
        png: include_bytes!("../assets/github-logo.png"),
        left: 403.0,
        aspect: 1.0324,
        url: REPO_URL,
    },
];
// The round Mute and Close buttons, side by side at the top right, over the
// corner of the marquee: dark discs with a white ring and icon so they stand
// out against the art, with a tooltip saying what they do. A web page can't
// close its tab, so there Mute takes Close's place in the corner.
const CLOSE_CENTER: egui::Pos2 = egui::pos2(480.0, 20.0);
#[cfg(not(target_arch = "wasm32"))]
const MUTE_CENTER: egui::Pos2 = egui::pos2(446.0, 20.0);
#[cfg(target_arch = "wasm32")]
const MUTE_CENTER: egui::Pos2 = CLOSE_CENTER;
const BUTTON_RADIUS: f32 = 14.0;
// Soft drop shadow under each button: SHADOW_STEPS translucent black discs,
// growing to SHADOW_SPREAD past the button, nudged down by SHADOW_OFFSET.
const SHADOW_SPREAD: f32 = 7.0;
const SHADOW_STEPS: usize = 7;
const SHADOW_STEP_ALPHA: u8 = 22;
const SHADOW_OFFSET: egui::Vec2 = egui::vec2(0.0, 2.0);
const BUTTON_FILL: egui::Color32 = egui::Color32::from_black_alpha(200);
const BUTTON_HOVER_FILL: egui::Color32 = egui::Color32::from_gray(70);
#[cfg(not(target_arch = "wasm32"))]
const CLOSE_HOVER_FILL: egui::Color32 = egui::Color32::from_rgb(200, 40, 40);
const BUTTON_INK: egui::Color32 = egui::Color32::WHITE;
const MUTED_MARK: egui::Color32 = egui::Color32::from_rgb(240, 80, 80);
// eframe storage key for the mute setting.
const MUTED_KEY: &str = "muted";

const LINK_ICON_TOP: f32 = 625.0;
const LINK_ICON_SIZE: f32 = 50.0;
// The PNGs' margin, in design points (8px at 4x).
const LINK_ICON_MARGIN: f32 = 2.0;
const LINK_ICON_COLOR: egui::Color32 = egui::Color32::from_gray(75);
const LINK_ICON_HOVER_COLOR: egui::Color32 = egui::Color32::from_gray(125);

impl LinkIcon {
    // The square the PNG (minus its margin) is drawn into.
    fn box_rect(&self) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(self.left, LINK_ICON_TOP), egui::Vec2::splat(LINK_ICON_SIZE))
    }

    // The drawing itself, centered in the box: the clickable area.
    fn drawing_rect(&self) -> egui::Rect {
        let size = if self.aspect >= 1.0 {
            egui::vec2(LINK_ICON_SIZE, LINK_ICON_SIZE / self.aspect)
        } else {
            egui::vec2(LINK_ICON_SIZE * self.aspect, LINK_ICON_SIZE)
        };
        egui::Rect::from_center_size(self.box_rect().center(), size)
    }
}

// A round button at `center` (window coordinates): draws the disc and ring,
// then `icon` inside, and returns the response with `tooltip` attached.
fn round_button(
    ui: &egui::Ui,
    id: &str,
    center: egui::Pos2,
    tooltip: &str,
    hover_fill: egui::Color32,
    icon: impl FnOnce(&egui::Painter, egui::Pos2),
) -> egui::Response {
    let rect = egui::Rect::from_center_size(center, egui::Vec2::splat(BUTTON_RADIUS * 2.0));
    let resp = ui.interact(rect, egui::Id::new(id), egui::Sense::click()).on_hover_text(tooltip);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let painter = ui.painter();
    // Largest, faintest disc first; the overlaps darken toward the middle.
    for step in (1..=SHADOW_STEPS).rev() {
        let radius = BUTTON_RADIUS + SHADOW_SPREAD * step as f32 / SHADOW_STEPS as f32;
        painter.circle_filled(center + SHADOW_OFFSET, radius, egui::Color32::from_black_alpha(SHADOW_STEP_ALPHA));
    }
    painter.circle(
        center,
        BUTTON_RADIUS,
        if resp.hovered() { hover_fill } else { BUTTON_FILL },
        egui::Stroke::new(1.5f32, BUTTON_INK),
    );
    icon(painter, center);
    resp
}

#[cfg(not(target_arch = "wasm32"))]
fn draw_close_icon(painter: &egui::Painter, c: egui::Pos2) {
    let stroke = egui::Stroke::new(2.5f32, BUTTON_INK);
    let d = 5.5;
    painter.line_segment([c + egui::vec2(-d, -d), c + egui::vec2(d, d)], stroke);
    painter.line_segment([c + egui::vec2(d, -d), c + egui::vec2(-d, d)], stroke);
}

// A speaker; with sound waves, or a red X when muted.
fn draw_speaker_icon(painter: &egui::Painter, c: egui::Pos2, muted: bool) {
    let c = c + egui::vec2(-2.5, 0.0);
    let p = |x: f32, y: f32| c + egui::vec2(x, y);
    painter.add(egui::Shape::convex_polygon(
        vec![p(-7.0, -3.0), p(-3.5, -3.0), p(1.5, -7.5), p(1.5, 7.5), p(-3.5, 3.0), p(-7.0, 3.0)],
        BUTTON_INK,
        egui::Stroke::NONE,
    ));
    if muted {
        let stroke = egui::Stroke::new(2.0f32, MUTED_MARK);
        painter.line_segment([p(4.5, -3.5), p(11.5, 3.5)], stroke);
        painter.line_segment([p(11.5, -3.5), p(4.5, 3.5)], stroke);
    } else {
        for radius in [5.0, 9.0] {
            let points = (-4..=4)
                .map(|i| {
                    let a = i as f32 / 4.0 * std::f32::consts::FRAC_PI_4;
                    p(1.5 + radius * a.cos(), radius * a.sin())
                })
                .collect();
            painter.add(egui::Shape::line(points, egui::Stroke::new(1.8f32, BUTTON_INK)));
        }
    }
}

#[derive(PartialEq)]
enum Screen {
    CharSelect,
    TestYourMight,
}

pub(crate) fn load_texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    load_texture_with(ctx, name, bytes, egui::TextureOptions::LINEAR)
}

fn load_texture_with(
    ctx: &egui::Context,
    name: &str,
    bytes: &[u8],
    options: egui::TextureOptions,
) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let pixels = img.into_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
    Some(ctx.load_texture(name, color_image, options))
}

struct App {
    bg_texture: Option<egui::TextureHandle>,
    // LINK_ICONS' textures, in the same order.
    link_icons: Vec<Option<egui::TextureHandle>>,
    screen: Screen,
    char_select: CharSelectScreen,
    test_your_might: TestYourMightScreen,
    dev_screenshot: dev::Screenshot,
    #[cfg(not(target_arch = "wasm32"))]
    updater: UpdatePrompt,
    audio: Audio,
    #[cfg(not(target_arch = "wasm32"))]
    height_fraction: f32,
    #[cfg(not(target_arch = "wasm32"))]
    fit: WindowFit,
}

// fit_window's state between frames. Sizes are physical pixels.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct WindowFit {
    // Size the window should be, from the monitor it last settled on.
    target_px: Option<egui::Vec2>,
    // When we last asked the OS for target_px. A request can be lost (on
    // Windows, winit's own resize for a DPI change lands after ours), so it's
    // retried every RESIZE_RETRY_SECS while the window is still off.
    resize_requested_at: Option<f64>,
    // Zoom factor the current frame's input was gathered with (see
    // fit_window).
    input_zoom: Option<f32>,
    last_outer_px: Option<egui::Pos2>,
    last_wanted_px: Option<egui::Vec2>,
    // Last time the window moved or its monitor-based size changed.
    changed_at: f64,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, #[cfg(not(target_arch = "wasm32"))] height_fraction: f32) -> Self {
        let bg_texture = load_texture(&cc.egui_ctx, "cabinet-bg", include_bytes!("../assets/mk-cabinet.png"));
        let test_your_might = TestYourMightScreen::new(cc);
        // Drawn at a fraction of their size, so mipmapped to stay smooth.
        let link_icons = LINK_ICONS
            .iter()
            .map(|icon| {
                load_texture_with(
                    &cc.egui_ctx,
                    icon.id,
                    icon.png,
                    egui::TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..egui::TextureOptions::LINEAR },
                )
            })
            .collect();
        let mut app = Self {
            bg_texture,
            link_icons,
            screen: Screen::CharSelect,
            char_select: CharSelectScreen::new(cc, test_your_might.progress()),
            test_your_might,
            dev_screenshot: dev::Screenshot::from_env(),
            #[cfg(not(target_arch = "wasm32"))]
            updater: UpdatePrompt::start(&cc.egui_ctx),
            audio: Audio::open(cc.storage.and_then(|s| eframe::get_value(s, MUTED_KEY)).unwrap_or(false)),
            #[cfg(not(target_arch = "wasm32"))]
            height_fraction,
            #[cfg(not(target_arch = "wasm32"))]
            fit: WindowFit::default(),
        };
        // fit_window / fit_page owns the zoom factor.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        if let Some(character) = dev::select_character() {
            app.char_select.dev_select(character);
        }
        if let Some(character) = dev::start_character() {
            app.test_your_might.start_match(&cc.egui_ctx, &mut app.audio, character, 0.0);
            app.screen = Screen::TestYourMight;
        } else {
            app.audio.play(Sound::InsertCoin);
            app.audio.play_music(Sound::CharacterSelectTheme);
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

    // Sizes the window from the monitor it's on: HEIGHT_FRACTION (or
    // --height) of the monitor's height, cabinet-shaped, whatever the OS
    // scale setting. Everything is laid out in fixed 500x700 design points
    // and the zoom factor maps those onto the window, so the whole cabinet
    // scales together. Zoom always follows the window's actual size, so the
    // design stays in proportion even if the OS resizes the window (DPI
    // change mid-drag, a refused resize, a restored saved size). A monitor
    // change only resizes once the window position and the monitor reading
    // have both been steady for SETTLE_SECS, so it doesn't jump around
    // mid-drag or on a one-off odd reading (and still works on Wayland,
    // where the window position isn't known). Returns where the design sits
    // in the window, in points.
    #[cfg(not(target_arch = "wasm32"))]
    fn fit_window(&mut self, ctx: &egui::Context) -> egui::Rect {
        let now = ctx.input(|i| i.time);
        let ppp = ctx.pixels_per_point();
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        let fit = &mut self.fit;
        // egui-winit converts monitor_size/outer_rect to points with the zoom
        // in effect when it gathered this frame's input. After a zoom change
        // egui switches to the new zoom but only rescales screen_rect, so
        // converting them back with ctx.pixels_per_point() would be off by
        // the zoom ratio for a frame - enough to set off an endless resize
        // loop. Use the zoom the input was gathered with instead.
        let input_ppp = native * fit.input_zoom.unwrap_or(ctx.zoom_factor());
        let (monitor_px, outer_px, maximized, fullscreen) = ctx.input(|i| {
            let v = i.viewport();
            (v.monitor_size.map(|s| s * input_ppp), v.outer_rect.map(|r| r.min * input_ppp), v.maximized, v.fullscreen)
        });

        // A maximized/fullscreen window can't take our size.
        if maximized == Some(true) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        }
        if fullscreen == Some(true) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }

        // Physical pixels per design point for this monitor.
        let wanted_scale = monitor_px.map_or(native, |m| self.height_fraction * m.y / WINDOW_H);
        let wanted_px = (WINDOW_SIZE * wanted_scale).round();

        if outer_px != fit.last_outer_px || Some(wanted_px) != fit.last_wanted_px {
            fit.last_outer_px = outer_px;
            fit.last_wanted_px = Some(wanted_px);
            fit.changed_at = now;
        }
        // (update's regular repaint keeps checking this.)
        let settled = now - fit.changed_at >= SETTLE_SECS;
        if fit.target_px.is_none() || settled {
            fit.target_px = Some(wanted_px);
        }
        let target_px = fit.target_px.unwrap_or(wanted_px);

        let window_px = ctx.screen_rect().size() * ppp;
        let off = (window_px - target_px).abs().max_elem() > 1.5;
        let zoom = (window_px.x / WINDOW_W).min(window_px.y / WINDOW_H) / native;
        if !off {
            fit.resize_requested_at = None;
        } else if settled && fit.resize_requested_at.is_none_or(|t| now - t >= RESIZE_RETRY_SECS) {
            // InnerSize is in points at the zoom it's applied with, which is
            // still this frame's (set_zoom_factor applies next frame).
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_px / ctx.pixels_per_point()));
            fit.resize_requested_at = Some(now);
        }
        // The next frame's input is gathered with the zoom in effect now (a
        // zoom set below only applies once that frame starts).
        fit.input_zoom = Some(ctx.zoom_factor());
        if (zoom - ctx.zoom_factor()).abs() > 0.0001 {
            ctx.set_zoom_factor(zoom);
        }
        // Centered, snapped to whole pixels.
        let min = (ctx.screen_rect().center() - WINDOW_SIZE / 2.0) * ppp;
        egui::Rect::from_min_size(egui::pos2(min.x.round(), min.y.round()) / ppp, WINDOW_SIZE)
    }

    // The web page's version of fit_window: zooms so the cabinet fills as
    // much of the page as it can at its own proportions, PAGE_MARGIN in from
    // the top and sides. The art ends where the desktop window does, cut off
    // across the coin door, so it stands on the page's bottom edge: the
    // cabinet reads as carrying on below it rather than as chopped off. The
    // page's size is the browser's business, so it just follows it. Returns
    // where the design sits on the page, in points.
    #[cfg(target_arch = "wasm32")]
    fn fit_page(&mut self, ctx: &egui::Context) -> egui::Rect {
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        let room_px = ctx.screen_rect().size() * ctx.pixels_per_point() - egui::vec2(2.0, 1.0) * PAGE_MARGIN * native;
        let zoom = ((room_px.x / WINDOW_W).min(room_px.y / WINDOW_H) / native).max(0.1);
        if (zoom - ctx.zoom_factor()).abs() > 0.0001 {
            ctx.set_zoom_factor(zoom);
        }
        let ppp = ctx.pixels_per_point();
        let page = ctx.screen_rect();
        let min = egui::pos2(page.center().x - WINDOW_W / 2.0, page.max.y - WINDOW_H) * ppp;
        egui::Rect::from_min_size(egui::pos2(min.x.round(), min.y.round()) / ppp, WINDOW_SIZE)
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        #[cfg(not(target_arch = "wasm32"))]
        let window = self.fit_window(ctx);
        #[cfg(target_arch = "wasm32")]
        let window = self.fit_page(ctx);
        // Silent while another window has focus. (Unknown counts as focused.)
        self.audio.set_focused(ctx.input(|i| i.viewport().focused).unwrap_or(true));
        match self.screen {
            // An update offer (character select only) takes the keyboard
            // while it's up.
            #[cfg(not(target_arch = "wasm32"))]
            Screen::CharSelect if self.updater.active() => {
                if self.updater.handle_input(ctx) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Screen::CharSelect => {
                self.char_select.handle_input(ctx, &mut self.audio);
                if let Some(character) = self.char_select.chosen(ctx) {
                    let now = ctx.input(|i| i.time);
                    self.test_your_might.start_match(ctx, &mut self.audio, character, now);
                    self.screen = Screen::TestYourMight;
                }
            }
            Screen::TestYourMight => {
                if self.test_your_might.handle_input(ctx, &mut self.audio) == test_your_might::Action::CharacterSelect {
                    self.audio.stop_announcer();
                    self.audio.play_music(Sound::CharacterSelectTheme);
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
                        #[cfg(not(target_arch = "wasm32"))]
                        self.updater.draw(&screen_ui, screen_rect);
                    }
                    Screen::TestYourMight => {
                        self.test_your_might.draw(&mut screen_ui, ctx, screen_rect);
                    }
                }

                let muted = self.audio.muted();
                let mute_resp = round_button(
                    ui,
                    "mute-button",
                    MUTE_CENTER + window.min.to_vec2(),
                    if muted { "Unmute sound" } else { "Mute sound" },
                    BUTTON_HOVER_FILL,
                    |painter, c| draw_speaker_icon(painter, c, muted),
                );
                if mute_resp.clicked() {
                    self.audio.set_muted(!muted);
                }
                #[cfg(not(target_arch = "wasm32"))]
                let close_resp = round_button(
                    ui,
                    "close-button",
                    CLOSE_CENTER + window.min.to_vec2(),
                    "Close",
                    CLOSE_HOVER_FILL,
                    draw_close_icon,
                );
                #[cfg(not(target_arch = "wasm32"))]
                if close_resp.clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }

                let mut icon_rects = Vec::with_capacity(LINK_ICONS.len());
                for (icon, tex) in LINK_ICONS.iter().zip(&self.link_icons) {
                    let hit_rect = icon.drawing_rect().translate(window.min.to_vec2());
                    let resp = ui.interact(hit_rect, egui::Id::new(icon.id), egui::Sense::click());
                    let color = if resp.hovered() {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        LINK_ICON_HOVER_COLOR
                    } else {
                        LINK_ICON_COLOR
                    };
                    if let Some(tex) = tex {
                        ui.painter().image(
                            tex.id(),
                            icon.box_rect().translate(window.min.to_vec2()).expand(LINK_ICON_MARGIN),
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            color,
                        );
                    }
                    if resp.clicked() {
                        ctx.open_url(egui::OpenUrl::new_tab(icon.url));
                    }
                    icon_rects.push(hit_rect);
                }

                // Drag the whole (undecorated) window from anywhere on the
                // cabinet art outside the screen, the buttons, and the link
                // icons.
                #[cfg(not(target_arch = "wasm32"))]
                if ui.input(|i| i.pointer.primary_pressed()) {
                    let button_rects = [mute_resp.rect, close_resp.rect];
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        if !screen_rect.contains(pos)
                            && !button_rects.iter().any(|r| r.contains(pos))
                            && !icon_rects.iter().any(|r| r.contains(pos))
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                    }
                }
            });
        self.dev_screenshot.update(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, MUTED_KEY, &self.audio.muted());
    }

    // The page around the cabinet (index.html uses the same color).
    #[cfg(target_arch = "wasm32")]
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(PAGE_COLOR).to_array()
    }
}

// Release builds on Windows are GUI apps with no console of their own, so
// output from command-line flags would go nowhere. Attach to the console of
// the terminal that launched us, if any (a no-op when started from Explorer,
// and in debug builds, which already have a console).
#[cfg(windows)]
fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // SAFETY: plain Win32 call with a constant argument; failure is harmless.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    #[cfg(windows)]
    attach_parent_console();

    // Command-line flags:
    //   --version (-v)     print the version and exit
    //   --update           install the latest release, if newer, and exit
    //   --reset            delete the saved progress, then start fresh
    //   --height <frac>    window height as a fraction of the monitor's
    //                      height (default 0.5)
    // --version is checked first so it never has side effects like --reset.
    if std::env::args().skip(1).any(|a| matches!(a.as_str(), "--version" | "--v" | "-v" | "-V")) {
        println!("Test Your Might {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if std::env::args().skip(1).any(|a| a == "--update") {
        std::process::exit(update::run_cli());
    }
    let mut height_fraction = HEIGHT_FRACTION;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--height" => match args.next().and_then(|v| v.parse::<f32>().ok()) {
                Some(f) if (0.1..=1.0).contains(&f) => height_fraction = f,
                _ => eprintln!("--height needs a fraction between 0.1 and 1.0; using {HEIGHT_FRACTION}"),
            },
            "--reset" => match progress::Progress::delete_save() {
                Ok(Some(path)) => eprintln!("--reset: deleted {}", path.display()),
                Ok(None) => eprintln!("--reset: no saved progress to delete"),
                Err(e) => eprintln!("--reset: couldn't delete saved progress: {e}"),
            },
            other => eprintln!("ignoring unknown argument {other:?} (supported: --version, --update, --reset, --height <frac>)"),
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
        Box::new(move |cc| Ok(Box::new(App::new(cc, height_fraction)))),
    )
}

// The web page's background, around the cabinet: index.html's body color.
#[cfg(target_arch = "wasm32")]
const PAGE_COLOR: egui::Color32 = egui::Color32::from_rgb(11, 11, 14);

// On the web, `main` runs when the page loads the game (see index.html): it
// starts the game on the page's canvas and takes down the loading message.
#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().and_then(|w| w.document()).expect("the game runs in a web page");
        let canvas = document
            .get_element_by_id("game")
            .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .expect("index.html has a <canvas id=\"game\">");
        let result = eframe::WebRunner::new()
            .start(canvas.clone(), eframe::WebOptions::default(), Box::new(|cc| Ok(Box::new(App::new(cc)))))
            .await;
        let loading = document.get_element_by_id("loading");
        match result {
            Ok(()) => {
                if let Some(loading) = loading {
                    loading.remove();
                }
                // Keys only reach the game while its canvas has focus.
                let _ = canvas.focus();
            }
            Err(e) => {
                if let Some(loading) = loading {
                    loading.set_text_content(Some("The game couldn't start in this browser."));
                }
                warn!("the game couldn't start: {e:?}");
            }
        }
    });
}
