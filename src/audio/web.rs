// Web playback (see audio.rs): the browser's Web Audio API. Clips are decoded
// by rodio just like on the desktop, so looping and lead-in trimming are the
// same, then copied into AudioBuffers. Decoding runs in the background one
// clip per turn of the browser's event loop, so the page keeps drawing.
//
// Browsers only let a page make sound once the player has interacted with it
// (a key press, click, or tap). Until then the AudioContext is suspended and
// its clock stands still, so whatever was started before (Insert Coin and the
// character select theme) plays from the top on the first key press or click.
// Every one of those resumes it, in case the browser suspended it again.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, AudioNode, AudioScheduledSourceNode, GainNode,
};

use super::{Clip, Sound, ANNOUNCER_GAP, MUSIC_VOLUME};

// A decoded clip, ready to play: the samples in an AudioBuffer, and for
// looping music where the loop starts, in seconds.
#[derive(Clone)]
struct WebClip {
    buffer: AudioBuffer,
    loop_start: Option<f64>,
}

impl WebClip {
    fn new(ctx: &AudioContext, clip: &Clip) -> Result<WebClip, JsValue> {
        let ch = clip.channels.get() as usize;
        let frames = clip.samples.len() / ch;
        let rate = clip.rate.get();
        let buffer = ctx.create_buffer(ch as u32, frames as u32, rate as f32)?;
        // AudioBuffers keep each channel separately; the clip is interleaved.
        for c in 0..ch {
            let channel: Vec<f32> = clip.samples.iter().skip(c).step_by(ch).copied().collect();
            buffer.copy_to_channel(&channel, c as i32)?;
        }
        let loop_start = clip.loop_start.map(|sample| (sample / ch) as f64 / rate as f64);
        Ok(WebClip { buffer, loop_start })
    }
}

// Decoded clips (None: failed to decode), shared with the background
// decoding task.
type Clips = Rc<RefCell<HashMap<Sound, Option<WebClip>>>>;

// The audio graph: every clip goes into `master` (the mute switch), music
// through `music` (MUSIC_VOLUME) first.
struct Output {
    ctx: AudioContext,
    master: GainNode,
    music: GainNode,
}

impl Output {
    fn new() -> Result<Output, JsValue> {
        let ctx = AudioContext::new()?;
        let master = ctx.create_gain()?;
        master.connect_with_audio_node(&ctx.destination())?;
        let music = ctx.create_gain()?;
        music.gain().set_value(MUSIC_VOLUME);
        music.connect_with_audio_node(&master)?;
        Ok(Output { ctx, master, music })
    }

    // Starts `clip` at `when` (the AudioContext's clock, in seconds) into
    // `into`.
    fn start(&self, clip: &WebClip, into: &AudioNode, when: f64) -> Result<AudioBufferSourceNode, JsValue> {
        let source = self.ctx.create_buffer_source()?;
        source.set_buffer(Some(&clip.buffer));
        if let Some(loop_start) = clip.loop_start {
            source.set_loop(true);
            source.set_loop_start(loop_start);
            source.set_loop_end(clip.buffer.duration());
        }
        source.connect_with_audio_node(into)?;
        source.start_with_when(when)?;
        Ok(source)
    }
}

pub struct Audio {
    // None when the browser has no Web Audio.
    output: Option<Output>,
    clips: Clips,
    // The announcer sequence's lines and the music track playing now, if
    // any.
    announcer: Vec<AudioBufferSourceNode>,
    music: Option<AudioBufferSourceNode>,
    user_muted: bool,
    focused: bool,
}

impl Audio {
    // Sets up Web Audio, or runs silently without it.
    pub fn open(muted: bool) -> Self {
        let output = match Output::new() {
            Ok(output) => Some(output),
            Err(e) => {
                warn!("no audio output, playing without sound: {e:?}");
                None
            }
        };
        let clips = Clips::default();
        if let Some(output) = &output {
            resume_on_input(&output.ctx);
            decode_in_background(output.ctx.clone(), clips.clone());
        }
        let audio = Audio { output, clips, announcer: Vec::new(), music: None, user_muted: muted, focused: true };
        audio.update_silenced();
        audio
    }

    // `sound`, decoded. Normally the background task already has; if not
    // (it's needed right away), decode it here.
    fn clip(&self, sound: Sound) -> Option<WebClip> {
        let output = self.output.as_ref()?;
        if let Some(clip) = self.clips.borrow().get(&sound) {
            return clip.clone();
        }
        let clip = web_clip(&output.ctx, sound);
        self.clips.borrow_mut().entry(sound).or_insert(clip).clone()
    }

    pub fn play(&mut self, sound: Sound) {
        if let (Some(clip), Some(output)) = (self.clip(sound), &self.output) {
            if let Err(e) = output.start(&clip, &output.master, 0.0) {
                warn!("couldn't play {sound:?}: {e:?}");
            }
        }
    }

    // Plays `lines` one after another, ANNOUNCER_GAP apart, cutting off any
    // sequence still playing.
    pub fn announce(&mut self, lines: &[Sound]) {
        self.stop_announcer();
        let clips: Vec<(Sound, WebClip)> = lines.iter().filter_map(|&line| Some((line, self.clip(line)?))).collect();
        let Some(output) = &self.output else { return };
        let mut when = output.ctx.current_time();
        for (i, (line, clip)) in clips.iter().enumerate() {
            if i > 0 {
                when += ANNOUNCER_GAP.as_secs_f64();
            }
            match output.start(clip, &output.master, when) {
                Ok(source) => self.announcer.push(source),
                Err(e) => warn!("couldn't play {line:?}: {e:?}"),
            }
            when += clip.buffer.duration();
        }
    }

    pub fn stop_announcer(&mut self) {
        for line in self.announcer.drain(..) {
            stop(&line);
        }
    }

    // Starts `track` from the top (looping if it's a looped theme), replacing
    // whatever music was playing.
    pub fn play_music(&mut self, track: Sound) {
        self.stop_music();
        let Some(clip) = self.clip(track) else { return };
        let Some(output) = &self.output else { return };
        match output.start(&clip, &output.music, 0.0) {
            Ok(source) => self.music = Some(source),
            Err(e) => warn!("couldn't play {track:?}: {e:?}"),
        }
    }

    pub fn stop_music(&mut self) {
        if let Some(music) = self.music.take() {
            stop(&music);
        }
    }

    // The Mute button's setting (not whether focus has silenced things).
    pub fn muted(&self) -> bool {
        self.user_muted
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.user_muted = muted;
        self.update_silenced();
    }

    // Everything is silent while the page doesn't have focus.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.update_silenced();
    }

    fn update_silenced(&self) {
        if let Some(output) = &self.output {
            let silenced = self.user_muted || !self.focused;
            output.master.gain().set_value(if silenced { 0.0 } else { 1.0 });
        }
    }
}

fn stop(source: &AudioBufferSourceNode) {
    // Only fails if it was never started.
    let _ = AudioScheduledSourceNode::stop(source);
    let _ = source.disconnect();
}

fn web_clip(ctx: &AudioContext, sound: Sound) -> Option<WebClip> {
    let clip = sound.decode()?;
    WebClip::new(ctx, &clip).map_err(|e| warn!("couldn't load sound {sound:?}: {e:?}")).ok()
}

// Decodes every clip not decoded yet, soonest needed first, handing the
// browser back control between clips.
fn decode_in_background(ctx: AudioContext, clips: Clips) {
    wasm_bindgen_futures::spawn_local(async move {
        for sound in Sound::ALL {
            next_turn().await;
            if !clips.borrow().contains_key(&sound) {
                let clip = web_clip(&ctx, sound);
                clips.borrow_mut().entry(sound).or_insert(clip);
            }
        }
    });
}

// Resolves on a later turn of the browser's event loop (after it has had a
// chance to draw and handle input).
async fn next_turn() {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback(&resolve);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

// Resumes `ctx` on every key press, click, and tap: only those let a page
// start sound. Listens while capturing, so it runs before the game's own
// handlers can stop the event.
fn resume_on_input(ctx: &AudioContext) {
    let Some(window) = web_sys::window() else { return };
    let ctx = ctx.clone();
    let resume = Closure::<dyn Fn()>::new(move || {
        if ctx.state() != AudioContextState::Running {
            let _ = ctx.resume();
        }
    });
    for event in ["keydown", "pointerdown", "pointerup", "touchend"] {
        let _ = window.add_event_listener_with_callback_and_bool(event, resume.as_ref().unchecked_ref(), true);
    }
    // Needed for as long as the page is open.
    resume.forget();
}
