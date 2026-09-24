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

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{ChannelCount, Decoder, MixerDeviceSink, Player, SampleRate, Source};

use crate::fighter::Character;

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

// Output buffer, in frames: ~21-23ms at 44.1/48kHz, versus rodio's default
// ~46ms, so effects are heard close to the key press. Falls back to rodio's
// default if the device won't take it.
const OUTPUT_BUFFER_FRAMES: u32 = 1024;

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
            .map_err(|e| eprintln!("couldn't decode sound {self:?}: {e}"))
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
                None => eprintln!("{self:?} is too short to loop; playing it once"),
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

impl Clip {
    fn source(&self, muted: &Arc<AtomicBool>) -> ClipSource {
        ClipSource {
            muted: muted.clone(),
            samples: self.samples.clone(),
            channels: self.channels,
            rate: self.rate,
            loop_start: self.loop_start,
            pos: 0,
        }
    }
}

struct ClipSource {
    // Audio's mute switch: plays silence (but keeps going) while set.
    muted: Arc<AtomicBool>,
    samples: Arc<[f32]>,
    channels: ChannelCount,
    rate: SampleRate,
    loop_start: Option<usize>,
    pos: usize,
}

impl Iterator for ClipSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pos >= self.samples.len() {
            self.pos = self.loop_start?;
        }
        self.pos += 1;
        Some(if self.muted.load(Ordering::Relaxed) { 0.0 } else { self.samples[self.pos - 1] })
    }
}

impl Source for ClipSource {
    fn current_span_len(&self) -> Option<usize> {
        // One span: the format never changes.
        None
    }

    fn channels(&self) -> ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> SampleRate {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        if self.loop_start.is_some() {
            return None;
        }
        let frames = self.samples.len() / self.channels.get() as usize;
        Some(Duration::from_secs_f64(frames as f64 / self.rate.get() as f64))
    }
}

pub struct Audio {
    // None when there's no output device.
    device: Option<MixerDeviceSink>,
    // Decoded clips (None: failed to decode), shared with the startup
    // decoding thread.
    clips: Arc<Mutex<HashMap<Sound, Option<Arc<Clip>>>>>,
    // The announcer sequence and music track playing now, if any; dropping
    // one stops it.
    announcer: Option<Player>,
    music: Option<Player>,
    // What every playing clip checks: user_muted or !focused.
    silenced: Arc<AtomicBool>,
    user_muted: bool,
    focused: bool,
}

impl Audio {
    // Opens the default output device, or runs silently without one.
    pub fn open(muted: bool) -> Self {
        let small_buffer = rodio::DeviceSinkBuilder::from_default_device().and_then(|builder| {
            builder.with_buffer_size(rodio::cpal::BufferSize::Fixed(OUTPUT_BUFFER_FRAMES)).open_stream()
        });
        let device = match small_buffer.or_else(|_| rodio::DeviceSinkBuilder::open_default_sink()) {
            Ok(mut device) => {
                device.log_on_drop(false);
                Some(device)
            }
            Err(e) => {
                eprintln!("no audio output, playing without sound: {e}");
                None
            }
        };
        let clips = Arc::new(Mutex::new(HashMap::new()));
        if device.is_some() {
            let clips = clips.clone();
            std::thread::spawn(move || {
                for sound in Sound::ALL {
                    if !clips.lock().unwrap().contains_key(&sound) {
                        let clip = sound.decode().map(Arc::new);
                        clips.lock().unwrap().entry(sound).or_insert(clip);
                    }
                }
            });
        }
        Audio {
            device,
            clips,
            announcer: None,
            music: None,
            silenced: Arc::new(AtomicBool::new(muted)),
            user_muted: muted,
            focused: true,
        }
    }

    // `sound`, decoded. Normally the startup thread already has; if not
    // (it's needed right away), decode it here.
    fn clip(&self, sound: Sound) -> Option<Arc<Clip>> {
        self.device.as_ref()?;
        if let Some(clip) = self.clips.lock().unwrap().get(&sound) {
            return clip.clone();
        }
        let clip = sound.decode().map(Arc::new);
        self.clips.lock().unwrap().entry(sound).or_insert(clip).clone()
    }

    pub fn play(&mut self, sound: Sound) {
        if let (Some(clip), Some(device)) = (self.clip(sound), &self.device) {
            device.mixer().add(clip.source(&self.silenced));
        }
    }

    // Plays `lines` one after another, ANNOUNCER_GAP apart, cutting off any
    // sequence still playing.
    pub fn announce(&mut self, lines: &[Sound]) {
        let clips: Vec<Arc<Clip>> = lines.iter().filter_map(|&line| self.clip(line)).collect();
        let Some(device) = &self.device else { return };
        let player = Player::connect_new(device.mixer());
        for (i, clip) in clips.iter().enumerate() {
            player.append(clip.source(&self.silenced).delay(if i == 0 { Duration::ZERO } else { ANNOUNCER_GAP }));
        }
        self.announcer = Some(player);
    }

    pub fn stop_announcer(&mut self) {
        self.announcer = None;
    }

    // Starts `track` from the top (looping if it's a looped theme), replacing
    // whatever music was playing.
    pub fn play_music(&mut self, track: Sound) {
        self.music = None;
        let Some(clip) = self.clip(track) else { return };
        let Some(device) = &self.device else { return };
        let player = Player::connect_new(device.mixer());
        player.set_volume(MUSIC_VOLUME);
        player.append(clip.source(&self.silenced));
        self.music = Some(player);
    }

    pub fn stop_music(&mut self) {
        self.music = None;
    }

    // The Mute button's setting (not whether focus has silenced things).
    pub fn muted(&self) -> bool {
        self.user_muted
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.user_muted = muted;
        self.update_silenced();
    }

    // Everything is silent while the window doesn't have focus.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.update_silenced();
    }

    #[cfg(test)]
    fn silenced(&self) -> bool {
        self.silenced.load(Ordering::Relaxed)
    }

    fn update_silenced(&self) {
        self.silenced.store(self.user_muted || !self.focused, Ordering::Relaxed);
    }
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

    #[test]
    fn character_select_theme_loops_seamlessly() {
        let clip = Sound::CharacterSelectTheme.decode().unwrap();
        let ch = clip.channels.get() as usize;
        assert_eq!(clip.loop_start, Some(CS_THEME_LOOP_START * ch));
        assert_eq!(clip.samples.len(), (CS_THEME_LOOP_START + CS_THEME_LOOP_FRAMES) * ch);
        // Playing across the seam: the jump from the loop's end back to its
        // start is no bigger than the track's ordinary sample-to-sample steps.
        let total = clip.samples.len() + 10 * ch;
        let played: Vec<f32> = clip.source(&Arc::default()).take(total).collect();
        assert_eq!(played.len(), total, "a looped theme never ends");
        let step = |i: usize| (played[i] - played[i - ch]).abs();
        let seam = clip.samples.len();
        let typical = (seam - 1000 * ch..seam).map(step).fold(0.0, f32::max);
        assert!(step(seam) <= typical, "seam step {} vs max step {typical}", step(seam));
    }

    #[test]
    fn muting_silences_without_losing_the_place() {
        let clip = Sound::CharacterSelectTheme.decode().unwrap();
        let muted = Arc::new(AtomicBool::new(false));
        let mut source = clip.source(&muted);
        let n = 44_100;
        let before: Vec<f32> = source.by_ref().take(n).collect();
        assert!(before.iter().any(|&s| s != 0.0));
        muted.store(true, Ordering::Relaxed);
        let during: Vec<f32> = source.by_ref().take(n).collect();
        assert!(during.iter().all(|&s| s == 0.0));
        muted.store(false, Ordering::Relaxed);
        let expected = &clip.samples[2 * n..3 * n];
        assert_eq!(source.take(n).collect::<Vec<_>>(), expected);
    }

    #[test]
    fn silent_when_muted_or_unfocused() {
        // No output device needed for the switch logic.
        let mut audio = Audio {
            device: None,
            clips: Arc::default(),
            announcer: None,
            music: None,
            silenced: Arc::default(),
            user_muted: false,
            focused: true,
        };
        assert!(!audio.silenced());
        audio.set_focused(false);
        assert!(audio.silenced() && !audio.muted());
        audio.set_muted(true);
        audio.set_focused(true);
        assert!(audio.silenced() && audio.muted());
        audio.set_muted(false);
        assert!(!audio.silenced());
    }

    #[test]
    fn one_shot_music_ends() {
        let clip = Sound::TestYourMightTheme.decode().unwrap();
        assert_eq!(clip.loop_start, None);
        assert_eq!(clip.source(&Arc::default()).count(), clip.samples.len());
    }
}
