// Fighters: the 7 playable characters, their 9-frame sprite sheets, and the
// frame-sequencing state machine shared by player 1 and the CPU opponent.
//
// Every character has frames 01-09 (182x224, feet at the same baseline):
//   1-4  idle    - loops 1,2,3,4,1,2,... forever
//   5-7  strike  - plays once, then holds on 7 (until the test is passed)
//   8-9  victory - plays once, then holds on 9

use eframe::egui;

use crate::load_texture;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Character {
    JohnnyCage,
    Kano,
    Scorpion,
    SonyaBlade,
    Raiden,
    LiuKang,
    SubZero,
}

impl Character {
    pub const ALL: [Character; 7] = [
        Character::JohnnyCage,
        Character::Kano,
        Character::Scorpion,
        Character::SonyaBlade,
        Character::Raiden,
        Character::LiuKang,
        Character::SubZero,
    ];

    fn sprite_prefix(self) -> &'static str {
        match self {
            Character::JohnnyCage => "johnny_cage",
            Character::Kano => "kano",
            Character::Scorpion => "scorpion",
            Character::SonyaBlade => "sonya_blade",
            Character::Raiden => "raiden",
            Character::LiuKang => "liu_kang",
            Character::SubZero => "sub_zero",
        }
    }

    fn frame_bytes(self) -> &'static [&'static [u8]; FRAME_COUNT] {
        macro_rules! frames {
            ($name:literal) => {
                &[
                    include_bytes!(concat!("../assets/", $name, "_frame01.png")),
                    include_bytes!(concat!("../assets/", $name, "_frame02.png")),
                    include_bytes!(concat!("../assets/", $name, "_frame03.png")),
                    include_bytes!(concat!("../assets/", $name, "_frame04.png")),
                    include_bytes!(concat!("../assets/", $name, "_frame05.png")),
                    include_bytes!(concat!("../assets/", $name, "_frame06.png")),
                    include_bytes!(concat!("../assets/", $name, "_frame07.png")),
                    include_bytes!(concat!("../assets/", $name, "_frame08.png")),
                    include_bytes!(concat!("../assets/", $name, "_frame09.png")),
                ]
            };
        }
        match self {
            Character::JohnnyCage => frames!("johnny_cage"),
            Character::Kano => frames!("kano"),
            Character::Scorpion => frames!("scorpion"),
            Character::SonyaBlade => frames!("sonya_blade"),
            Character::Raiden => frames!("raiden"),
            Character::LiuKang => frames!("liu_kang"),
            Character::SubZero => frames!("sub_zero"),
        }
    }

    // A uniformly random character that is never `self`.
    pub fn random_opponent(self) -> Character {
        use rand::seq::IteratorRandom;
        Character::ALL
            .into_iter()
            .filter(|&c| c != self)
            .choose(&mut rand::thread_rng())
            .unwrap()
    }
}

pub const FRAME_COUNT: usize = 9;

// How long each sprite frame is shown before advancing.
const FRAME_SECS: f64 = 0.2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pose {
    Idle,
    // Not triggered yet - the strike/victory flow is wired up once the
    // pass/fail rules for the test are defined.
    #[allow(dead_code)]
    Strike,
    #[allow(dead_code)]
    Victory,
}

impl Pose {
    // 0-based frame indices for this pose, in playback order.
    fn frames(self) -> &'static [usize] {
        match self {
            Pose::Idle => &[0, 1, 2, 3],
            Pose::Strike => &[4, 5, 6],
            Pose::Victory => &[7, 8],
        }
    }

    fn loops(self) -> bool {
        self == Pose::Idle
    }

    // 0-based frame index to show `elapsed` seconds after entering this pose.
    // Looping poses wrap; one-shot poses hold on their last frame.
    pub fn frame_at(self, elapsed: f64) -> usize {
        let frames = self.frames();
        let step = (elapsed.max(0.0) / FRAME_SECS) as usize;
        let i = if self.loops() { step % frames.len() } else { step.min(frames.len() - 1) };
        frames[i]
    }
}

pub struct Fighter {
    frames: [Option<egui::TextureHandle>; FRAME_COUNT],
    pose: Pose,
    pose_started: f64,
}

impl Fighter {
    // `now` is egui's input time (seconds), used as the pose's start.
    pub fn new(ctx: &egui::Context, character: Character, now: f64) -> Self {
        let prefix = character.sprite_prefix();
        let bytes = character.frame_bytes();
        let frames = std::array::from_fn(|i| load_texture(ctx, &format!("{prefix}_frame{:02}", i + 1), bytes[i]));
        Self { frames, pose: Pose::Idle, pose_started: now }
    }

    #[allow(dead_code)]
    pub fn set_pose(&mut self, pose: Pose, now: f64) {
        if self.pose != pose {
            self.pose = pose;
            self.pose_started = now;
        }
    }

    // Draws the current frame at native size with its canvas bottom-center on
    // `foot` (screen coordinates).
    pub fn draw(&self, painter: &egui::Painter, foot: egui::Pos2, now: f64) {
        let Some(tex) = &self.frames[self.pose.frame_at(now - self.pose_started)] else {
            return;
        };
        let size = tex.size_vec2();
        let rect = egui::Rect::from_min_size(egui::pos2(foot.x - size.x / 2.0, foot.y - size.y), size);
        painter.image(
            tex.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    // Seconds until the displayed frame next changes, or None if it's holding.
    pub fn next_frame_in(&self, now: f64) -> Option<f64> {
        let elapsed = (now - self.pose_started).max(0.0);
        let step = (elapsed / FRAME_SECS) as usize;
        if !self.pose.loops() && step + 1 >= self.pose.frames().len() {
            return None;
        }
        Some((step + 1) as f64 * FRAME_SECS - elapsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sequence(pose: Pose, steps: usize) -> Vec<usize> {
        (0..steps).map(|s| pose.frame_at((s as f64 + 0.5) * FRAME_SECS) + 1).collect()
    }

    #[test]
    fn idle_loops_1_to_4() {
        assert_eq!(sequence(Pose::Idle, 10), [1, 2, 3, 4, 1, 2, 3, 4, 1, 2]);
    }

    #[test]
    fn strike_holds_on_7() {
        assert_eq!(sequence(Pose::Strike, 6), [5, 6, 7, 7, 7, 7]);
    }

    #[test]
    fn victory_holds_on_9() {
        assert_eq!(sequence(Pose::Victory, 5), [8, 9, 9, 9, 9]);
    }

    #[test]
    fn opponent_is_never_the_player() {
        for player in Character::ALL {
            let mut seen = std::collections::HashSet::new();
            for _ in 0..500 {
                let cpu = player.random_opponent();
                assert_ne!(cpu, player);
                seen.insert(cpu);
            }
            assert_eq!(seen.len(), Character::ALL.len() - 1, "every other character should be reachable");
        }
    }
}
