// Sound effects, announcer lines and music, from assets/sounds/ (see
// assets/sounds/SOURCES.md). Three kinds of playback:
// - effects are fire and forget and overlap freely;
// - announcer lines play as one sequence at a time, which the next sequence
//   (or `stop_announcer`) cuts off, so lines never talk over each other;
// - music is one track at a time, looped or played once, under everything
//   else (MUSIC_VOLUME).
// Every clip is decoded once, in the background at startup (or on the spot if
// it's needed first), and kept in memory; effects have
// the MP3 encoder's near-silent lead-in trimmed so they land on the frame
// that triggers them. Muting (the Mute button, or the window losing focus)
// silences everything at once, sounds already playing included; music keeps
// its place, so unmuting picks it back up.
// With no audio output device the game just runs silently.
//
// Decoding (this file) is the same everywhere; playback is rodio on the
// desktop (audio/native.rs) and the browser's Web Audio on the web
// (audio/web.rs). Both are `Audio`, with the same methods.

use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use rodio::{ChannelCount, Decoder, SampleRate, Source};

use crate::fighter::Character;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Audio;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::Audio;


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sound {
    // App start, on character select.
    InsertCoin,
    // The character select cursor moving to another fighter.
    CursorMove,
    // Confirming a fighter on character select.
    FighterChosen,
    // Announcer, at the start of every round.
    TestYourMight,
    // A slab breaking.
    SlabBreak,
    // Announcer: "<fighter> wins", when the player breaks their slab.
    Wins(Character),
    // Announcer, after "<fighter> wins".
    Excellent,
    // Crowd, after "Excellent".
    Claps,
    // Music: character select, looped (see `Sound::music_loop`).
    CharacterSelectTheme,
    // Music: the start of every round, once.
    TestYourMightTheme,
}

// The character select theme is a gamerip that plays the tune once and fades
// out, so it's looped from inside: a 3.136s intro, then a 2-bar phrase of
// 180256 frames (4.087s, well before the fade at ~8.5s) repeated forever.
// Found by autocorrelating the decoded track - the phrase repeats with
// r = 0.98 across the seam - and CROSSFADE_FRAMES hide what's left.
const CS_THEME_LOOP_START: usize = 138_285;
const CS_THEME_LOOP_FRAMES: usize = 180_256;
const CROSSFADE_FRAMES: usize = 441;

// Leading samples quieter than this (about -50 dB) are the encoder's lead-in,
// not the sound; effects skip them.
const LEAD_IN_THRESHOLD: f32 = 0.003;

// The pause between lines in an announcer sequence. The clips have almost no
// silence at either end, so back to back they'd run together.
const ANNOUNCER_GAP: Duration = Duration::from_millis(250);

// The themes are mastered ~2 LU louder than the announcer and ~11 LU louder
// than the cursor click; at half volume (-6 dB) voices and effects sit on top.
const MUSIC_VOLUME: f32 = 0.5;
impl Sound {
    // Every sound, in the order they're decoded at startup: soonest needed
    // first.
    const ALL: [Sound; 16] = [
        Sound::InsertCoin,
        Sound::CharacterSelectTheme,
        Sound::CursorMove,
        Sound::FighterChosen,
        Sound::TestYourMightTheme,
        Sound::TestYourMight,
        Sound::SlabBreak,
        Sound::Wins(Character::JohnnyCage),
        Sound::Wins(Character::Kano),
        Sound::Wins(Character::Scorpion),
        Sound::Wins(Character::SonyaBlade),
        Sound::Wins(Character::Raiden),
        Sound::Wins(Character::LiuKang),
        Sound::Wins(Character::SubZero),
        Sound::Excellent,
        Sound::Claps,
    ];

    fn mp3(self) -> &'static [u8] {
        match self {
            Sound::InsertCoin => include_bytes!("../assets/sounds/insert-coin.mp3"),
            Sound::CursorMove => include_bytes!("../assets/sounds/ui-sound-1.mp3"),
            Sound::FighterChosen => include_bytes!("../assets/sounds/music-cue-1.mp3"),
            Sound::TestYourMight => include_bytes!("../assets/sounds/test-your-might.mp3"),
            Sound::SlabBreak => include_bytes!("../assets/sounds/hitsound-31.mp3"),
            Sound::Wins(character) => match character {
                Character::JohnnyCage => include_bytes!("../assets/sounds/johnny-cage-wins.mp3"),
                Character::Kano => include_bytes!("../assets/sounds/kano-wins.mp3"),
                Character::Scorpion => include_bytes!("../assets/sounds/scorpion-wins.mp3"),
                Character::SonyaBlade => include_bytes!("../assets/sounds/sonya-wins.mp3"),
                Character::Raiden => include_bytes!("../assets/sounds/raiden-wins.mp3"),
                Character::LiuKang => include_bytes!("../assets/sounds/liu-kang-wins.mp3"),
                Character::SubZero => include_bytes!("../assets/sounds/sub-zero-wins.mp3"),
            },
            Sound::Excellent => include_bytes!("../assets/sounds/excellent.mp3"),
            Sound::Claps => include_bytes!("../assets/sounds/claps-2.mp3"),
            Sound::CharacterSelectTheme => include_bytes!("../assets/sounds/character-select-theme.mp3"),
            Sound::TestYourMightTheme => include_bytes!("../assets/sounds/test-your-might-theme.mp3"),
        }
    }

    fn is_music(self) -> bool {
        matches!(self, Sound::CharacterSelectTheme | Sound::TestYourMightTheme)
    }

    // (loop start, loop length) in frames, for music that loops.
    fn music_loop(self) -> Option<(usize, usize)> {
        (self == Sound::CharacterSelectTheme).then_some((CS_THEME_LOOP_START, CS_THEME_LOOP_FRAMES))
    }

    fn decode(self) -> Option<Clip> {
        let decoder = Decoder::try_from(Cursor::new(self.mp3()))
            .map_err(|e| warn!("couldn't decode sound {self:?}: {e}"))
            .ok()?;
        let (channels, rate) = (decoder.channels(), decoder.sample_rate());
        let mut samples: Vec<f32> = decoder.collect();
        let ch = channels.get() as usize;
        let mut loop_start = None;
        if let Some((start, frames)) = self.music_loop() {
            match looped(&samples, ch, start, frames) {
                Some(body) => {
                    samples = body;
                    loop_start = Some(start * ch);
                }
                None => warn!("{self:?} is too short to loop; playing it once"),
            }
        } else if !self.is_music() {
            let first = samples.iter().position(|s| s.abs() > LEAD_IN_THRESHOLD).unwrap_or(0);
            samples.drain(..first - first % ch);
        }
        Some(Clip { samples: samples.into(), channels, rate, loop_start })
    }
}

// `samples` cut to end at the loop's end, with the loop's last
// CROSSFADE_FRAMES blended into the frames just before its start, so jumping
// from the end back to `start` is seamless. None if the track is too short.
fn looped(samples: &[f32], ch: usize, start: usize, frames: usize) -> Option<Vec<f32>> {
    let end = start + frames;
    if start < CROSSFADE_FRAMES || end * ch > samples.len() {
        return None;
    }
    let mut body = samples[..end * ch].to_vec();
    for f in 0..CROSSFADE_FRAMES {
        let w = (f + 1) as f32 / CROSSFADE_FRAMES as f32;
        for c in 0..ch {
            let tail = (end - CROSSFADE_FRAMES + f) * ch + c;
            let lead = (start - CROSSFADE_FRAMES + f) * ch + c;
            body[tail] = samples[tail] * (1.0 - w) + samples[lead] * w;
        }
    }
    Some(body)
}

// A decoded sound: interleaved samples, and for looping music the sample
// index playback jumps back to at the end.
struct Clip {
    samples: Arc<[f32]>,
    channels: ChannelCount,
    rate: SampleRate,
    loop_start: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_decodes() {
        for sound in Sound::ALL {
            let clip = sound.decode().unwrap_or_else(|| panic!("{sound:?} should decode"));
            let secs = clip.samples.len() as f64 / (clip.rate.get() as f64 * clip.channels.get() as f64);
            assert!(secs > 0.3, "{sound:?} is only {secs}s");
        }
    }

    #[test]
    fn every_fighter_has_a_win_line() {
        for character in Character::ALL {
            assert!(Sound::ALL.contains(&Sound::Wins(character)), "{character:?}");
        }
    }

    #[test]
    fn effects_start_on_the_sound() {
        let clip = Sound::CursorMove.decode().unwrap();
        let ch = clip.channels.get() as usize;
        assert!(clip.samples[..ch].iter().any(|s| s.abs() > LEAD_IN_THRESHOLD));
        assert_eq!(clip.samples.len() % ch, 0);
    }
}
