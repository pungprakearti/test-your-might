// Player progression, saved to disk: which material the player is currently
// trying to break, and the history of finished runs. The target WPM (the red
// bar on the gauge) comes from the average of the last few runs, reduced by a
// per-material percentage so easier materials are more forgiving.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

use crate::fighter::{Character, Unlock};

// Target before there's any history to average.
pub const START_TARGET_WPM: f64 = 5.0;
// How many of the most recent runs are averaged to set the target.
const AVERAGE_OF: usize = 5;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Material {
    #[default]
    Wood,
    Stone,
    Steel,
    Ruby,
    Diamond,
}

impl Material {
    pub const ALL: [Material; 5] = [Material::Wood, Material::Stone, Material::Steel, Material::Ruby, Material::Diamond];

    // Where the red target bar sits, as a fraction of the gauge height from
    // the bottom. Easier materials put it lower, so hitting the target feels
    // like a big surge up the gauge.
    pub fn bar_fraction(self) -> f32 {
        match self {
            Material::Wood => 0.35,
            Material::Stone => 0.50,
            Material::Steel => 0.65,
            Material::Ruby => 0.75,
            Material::Diamond => 0.85,
        }
    }

    // How far below the recent average the target is set. Diamond is the
    // hardest (only 5% below); easier materials get a bigger reduction.
    pub fn target_reduction(self) -> f64 {
        match self {
            Material::Wood => 0.25,
            Material::Stone => 0.20,
            Material::Steel => 0.15,
            Material::Ruby => 0.10,
            Material::Diamond => 0.05,
        }
    }

    // The material after this one is broken. Diamond is the last.
    pub fn next(self) -> Material {
        match self {
            Material::Wood => Material::Stone,
            Material::Stone => Material::Steel,
            Material::Steel => Material::Ruby,
            Material::Ruby | Material::Diamond => Material::Diamond,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Material::Wood => "WOOD",
            Material::Stone => "STONE",
            Material::Steel => "STEEL",
            Material::Ruby => "RUBY",
            Material::Diamond => "DIAMOND",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Run {
    pub wpm: f64,
    pub accuracy: f64,
    pub material: Material,
    pub target_wpm: f64,
    pub passed: bool,
    // Unix time (seconds) the run finished.
    pub finished_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Progress {
    pub material: Material,
    pub runs: Vec<Run>,
}

impl Progress {
    // The WPM needed to break the current material: the average of the last
    // AVERAGE_OF runs minus the material's reduction, never below the starting
    // target. Rounded to a whole WPM, since that's what's shown on screen.
    pub fn target_wpm(&self) -> f64 {
        let recent: Vec<f64> = self.runs.iter().rev().take(AVERAGE_OF).map(|r| r.wpm).collect();
        if recent.is_empty() {
            return START_TARGET_WPM;
        }
        let avg = recent.iter().sum::<f64>() / recent.len() as f64;
        (avg * (1.0 - self.material.target_reduction())).round().max(START_TARGET_WPM)
    }

    // Records a finished run against `target_wpm` (the target shown during
    // it). Beating it advances to the next material; falling short sends the
    // player back to wood to start the climb over (the goal still comes from
    // their recent average). Compared on the whole-number WPM the player sees.
    pub fn record(&mut self, wpm: f64, accuracy: f64, target_wpm: f64) -> Run {
        let passed = wpm.round() >= target_wpm;
        let run = Run { wpm, accuracy, material: self.material, target_wpm, passed, finished_at: unix_now() };
        self.runs.push(run.clone());
        self.material = if passed { self.material.next() } else { Material::Wood };
        run
    }

    // Whether this material has ever been broken.
    pub fn has_broken(&self, material: Material) -> bool {
        self.runs.iter().any(|r| r.material == material && r.passed)
    }

    // How many different local calendar days have a finished round.
    pub fn days_played(&self) -> usize {
        self.runs.iter().filter_map(|r| local_date(r.finished_at)).collect::<HashSet<_>>().len()
    }

    pub fn is_unlocked(&self, character: Character) -> bool {
        match character.unlock() {
            Unlock::Default => true,
            Unlock::Break(material) => self.has_broken(material),
            Unlock::PlayedDays(days) => self.days_played() >= days,
        }
    }

    // Loads saved progress, or starts fresh if there is none (or it can't be
    // read).
    pub fn load() -> Progress {
        let Some(path) = save_path() else {
            return Progress::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
                // Move it aside rather than let the next save overwrite it, so
                // the history can still be recovered by hand.
                let backup = path.with_extension(format!("json.unreadable-{}", unix_now()));
                eprintln!("progress: can't parse {} ({e}); moving it to {}", path.display(), backup.display());
                if let Err(e) = std::fs::rename(&path, &backup) {
                    eprintln!("progress: couldn't move it aside: {e}");
                }
                Progress::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Progress::default(),
            Err(e) => {
                eprintln!("progress: can't read {}: {e}", path.display());
                Progress::default()
            }
        }
    }

    // Permanently deletes the save file (for `--reset`). Returns the path it
    // deleted, or None if there was no save to delete.
    pub fn delete_save() -> std::io::Result<Option<PathBuf>> {
        let Some(path) = save_path() else {
            return Ok(None);
        };
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(Some(path)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self) {
        let Some(path) = save_path() else {
            eprintln!("progress: no data directory available, not saving");
            return;
        };
        if let Err(e) = write_atomically(&path, &serde_json::to_string_pretty(self).expect("progress serializes")) {
            eprintln!("progress: can't save {}: {e}", path.display());
        }
    }
}

fn local_date(unix_secs: u64) -> Option<chrono::NaiveDate> {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(i64::try_from(unix_secs).ok()?, 0).single().map(|t| t.date_naive())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default()
}

// progress.json in the OS data directory, or in $TYM_DATA_DIR when set (dev
// runs use this to keep test data away from real progress).
fn save_path() -> Option<PathBuf> {
    let dir = match std::env::var_os("TYM_DATA_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => directories::ProjectDirs::from("", "", "test-your-might")?.data_dir().to_path_buf(),
    };
    Some(dir.join("progress.json"))
}

// Writes via a temp file + rename so a crash mid-write can't corrupt the save.
fn write_atomically(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress_with(material: Material, wpms: &[f64]) -> Progress {
        let runs = wpms
            .iter()
            .map(|&wpm| Run { wpm, accuracy: 100.0, material, target_wpm: 0.0, passed: false, finished_at: 0 })
            .collect();
        Progress { material, runs }
    }

    #[test]
    fn starts_on_wood_at_5_wpm() {
        let p = Progress::default();
        assert_eq!(p.material, Material::Wood);
        assert_eq!(p.target_wpm(), START_TARGET_WPM);
    }

    #[test]
    fn target_is_reduced_average_of_last_5_runs() {
        // Only the last 5 (40, 40, 40, 60, 60 -> avg 48) count.
        let p = progress_with(Material::Wood, &[1000.0, 40.0, 40.0, 40.0, 60.0, 60.0]);
        assert_eq!(p.target_wpm(), 36.0); // 48 * 0.75
        let p = progress_with(Material::Diamond, &[40.0, 40.0, 40.0, 60.0, 60.0]);
        assert_eq!(p.target_wpm(), 46.0); // 48 * 0.95 = 45.6
    }

    #[test]
    fn target_never_drops_below_start() {
        let p = progress_with(Material::Wood, &[3.0, 4.0]);
        assert_eq!(p.target_wpm(), START_TARGET_WPM);
    }

    #[test]
    fn harder_materials_have_higher_bars_and_smaller_reductions() {
        for pair in Material::ALL.windows(2) {
            assert!(pair[1].bar_fraction() > pair[0].bar_fraction());
            assert!(pair[1].target_reduction() < pair[0].target_reduction());
        }
        assert_eq!(Material::Diamond.target_reduction(), 0.05);
    }

    #[test]
    fn failing_sends_you_back_to_wood_keeping_your_average() {
        let mut p = Progress::default();
        for _ in 0..3 {
            p.record(40.0, 100.0, 5.0);
        }
        assert_eq!(p.material, Material::Ruby);
        let goal_before = p.target_wpm();
        let run = p.record(10.0, 100.0, goal_before);
        assert!(!run.passed);
        assert_eq!(p.material, Material::Wood);
        // Average of 40, 40, 40, 10 = 32.5, less wood's 25%.
        assert_eq!(p.target_wpm(), (32.5_f64 * 0.75).round());
    }

    #[test]
    fn passing_advances_material_and_failing_wood_stays_on_wood() {
        let mut p = Progress::default();
        let run = p.record(4.0, 90.0, 5.0);
        assert!(!run.passed);
        assert_eq!(p.material, Material::Wood);
        let run = p.record(4.5, 90.0, 5.0); // shows as 5 WPM
        assert!(run.passed);
        assert_eq!(p.material, Material::Stone);
        assert_eq!(p.runs.len(), 2);
    }

    #[test]
    fn diamond_is_the_last_material() {
        let mut m = Material::Wood;
        for expected in [Material::Stone, Material::Steel, Material::Ruby, Material::Diamond, Material::Diamond] {
            m = m.next();
            assert_eq!(m, expected);
        }
    }

    fn run(material: Material, passed: bool, finished_at: u64) -> Run {
        Run { wpm: 50.0, accuracy: 100.0, material, target_wpm: 40.0, passed, finished_at }
    }

    #[test]
    fn only_liu_kang_is_unlocked_at_first() {
        let p = Progress::default();
        for c in Character::ALL {
            assert_eq!(p.is_unlocked(c), c == Character::LiuKang, "{c:?}");
        }
    }

    #[test]
    fn breaking_a_material_unlocks_its_fighter() {
        let mut p = Progress::default();
        p.runs.push(run(Material::Stone, false, 0));
        assert!(!p.is_unlocked(Character::Kano), "failing stone doesn't unlock Kano");
        p.runs.push(run(Material::Wood, true, 0));
        p.runs.push(run(Material::Stone, true, 0));
        let unlocked: Vec<Character> = Character::ALL.into_iter().filter(|&c| p.is_unlocked(c)).collect();
        assert_eq!(unlocked, [Character::JohnnyCage, Character::Kano, Character::LiuKang]);
    }

    #[test]
    fn scorpion_needs_ten_different_days() {
        const DAY: u64 = 24 * 60 * 60;
        let start = 1_790_000_000;
        let mut p = Progress::default();
        for day in 0..9 {
            // Several rounds on the same moment only count once.
            p.runs.push(run(Material::Wood, false, start + day * DAY));
            p.runs.push(run(Material::Wood, false, start + day * DAY));
        }
        // Days don't need to be in a row.
        p.runs.push(run(Material::Wood, false, start + 30 * DAY));
        assert_eq!(p.days_played(), 9 + 1);
        assert!(p.is_unlocked(Character::Scorpion));
        p.runs.pop();
        assert!(!p.is_unlocked(Character::Scorpion));
    }

    #[test]
    fn round_trips_through_json() {
        let mut p = Progress::default();
        p.record(42.0, 97.5, 5.0);
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Progress>(&json).unwrap(), p);
    }
}
