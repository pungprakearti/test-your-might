// Desktop playback (see audio.rs): rodio, mixing every clip into the default
// output device.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{ChannelCount, MixerDeviceSink, Player, SampleRate, Source};

use super::{Clip, Sound, ANNOUNCER_GAP, MUSIC_VOLUME};

// Output buffer, in frames: ~21-23ms at 44.1/48kHz, versus rodio's default
// ~46ms, so effects are heard close to the key press. Falls back to rodio's
// default if the device won't take it.
const OUTPUT_BUFFER_FRAMES: u32 = 1024;

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
                warn!("no audio output, playing without sound: {e}");
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
    use crate::audio::{CS_THEME_LOOP_FRAMES, CS_THEME_LOOP_START};

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
