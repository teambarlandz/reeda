//! Narration engine (docs/TTS_SPEC.md §4–§5): queue + state machine over
//! [`crate::chunk::NarrationChunk`]s, driven by a `TtsHost` platform.
//!
//! The engine is host-agnostic: the Android TextToSpeech bridge and the
//! desktop fake both implement `TtsHost` and deliver callbacks through
//! `HostEvent`s consumed via `TtsHost::poll`.

use std::collections::VecDeque;

use crate::chunk::NarrationChunk;

/// Narration engine states (TTS_SPEC §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// Not narrating.
    Idle,
    /// Chunks loaded, about to speak.
    Loading,
    /// Currently speaking (or queued).
    Speaking,
    /// Paused by the user.
    Paused,
    /// Unrecoverable error (3 consecutive utterance errors).
    Error,
}

/// A platform event fed back to the engine (utterance callbacks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEvent {
    /// `UtteranceProgressListener.onStart`.
    Started {
        /// Engine-assigned utterance id.
        utterance_id: u64,
    },
    /// `onRangeStart(start, end, …)` — char offsets within the chunk text.
    Range {
        /// Engine-assigned utterance id.
        utterance_id: u64,
        /// Start char offset in the chunk text.
        start: u32,
        /// End char offset (exclusive) in the chunk text.
        end: u32,
    },
    /// `onDone`.
    Done {
        /// Engine-assigned utterance id.
        utterance_id: u64,
    },
    /// `onError`.
    Error {
        /// Engine-assigned utterance id.
        utterance_id: u64,
    },
    /// A media-control action from the Android notification / lock screen
    /// (docs/TTS_SPEC.md §2), dispatched via the NarrationService.
    Control(ControlAction),
}

/// Media-control actions (notification buttons, TTS_SPEC §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    /// Resume playback.
    Play,
    /// Pause playback.
    Pause,
    /// Stop narration and tear down the foreground service.
    Stop,
    /// Jump to the previous chunk (chunk-level, per TTS_SPEC §4).
    SkipBack,
    /// Jump to the next chunk.
    SkipForward,
    /// Speech rate +0.1 (0.5–2.5, step 0.1, TTS_SPEC §4).
    SpeedUp,
    /// Speech rate −0.1.
    SpeedDown,
}

/// Effects the engine emits back to the caller (core App).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEffect {
    /// A word range is being spoken — highlight it.
    WordHighlight {
        /// Global block index.
        block_index: u32,
        /// Character offset within the block text.
        char_start: u32,
        /// End offset (exclusive).
        char_end: u32,
    },
    /// The final chunk finished — caller advances chapter or stops.
    Finished,
    /// Fatal error — engine is paused with a message.
    Error {
        /// Human-readable message.
        message: String,
    },
}

/// Host-side speech platform (Android TextToSpeech / desktop fake).
///
/// `Any` supertrait lets hosts be downcast (e.g. tests reaching the
/// [`FakeTtsHost`] inside an `App`).
pub trait TtsHost: std::any::Any {
    /// Enqueue `text` as the utterance `utterance_id` (QUEUE_ADD).
    fn speak(&mut self, utterance_id: u64, text: &str) -> Result<(), String>;
    /// Stop all speech (engine stop / error path).
    fn stop(&mut self) -> Result<(), String>;
    /// Pause speech (best-effort; Android pauses the queue).
    fn pause(&mut self) -> Result<(), String>;
    /// Resume paused speech.
    fn resume(&mut self) -> Result<(), String>;
    /// Set the speech rate (0.5–2.5).
    fn set_rate(&mut self, rate: f32) -> Result<(), String>;
    /// Set the pitch (0.5–1.5).
    fn set_pitch(&mut self, pitch: f32) -> Result<(), String>;
    /// Drain pending host events (binder-thread callbacks on Android).
    fn poll(&mut self) -> Vec<HostEvent>;
}

/// In-memory host for desktop builds and unit tests (records calls, events
/// injected manually via [`FakeTtsHost::push_event`]).
#[derive(Debug, Default)]
pub struct FakeTtsHost {
    spoken: Vec<(u64, String)>,
    events: VecDeque<HostEvent>,
    rate: f32,
    pitch: f32,
    stopped: usize,
    paused: usize,
    resumed: usize,
}

impl FakeTtsHost {
    /// Create an empty fake host.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inject a platform callback (simulates the device).
    pub fn push_event(&mut self, event: HostEvent) {
        self.events.push_back(event);
    }

    /// Utterances spoken so far, in order.
    pub fn spoken(&self) -> &[(u64, String)] {
        &self.spoken
    }

    /// How often `stop` was called.
    pub fn stop_count(&self) -> usize {
        self.stopped
    }

    /// How often `pause` was called.
    pub fn pause_count(&self) -> usize {
        self.paused
    }

    /// How often `resume` was called.
    pub fn resume_count(&self) -> usize {
        self.resumed
    }

    /// Last rate passed to `set_rate`.
    pub fn rate(&self) -> f32 {
        self.rate
    }

    /// Last pitch passed to `set_pitch`.
    pub fn pitch(&self) -> f32 {
        self.pitch
    }
}

impl TtsHost for FakeTtsHost {
    fn speak(&mut self, utterance_id: u64, text: &str) -> Result<(), String> {
        self.spoken.push((utterance_id, text.to_string()));
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.stopped += 1;
        Ok(())
    }

    fn pause(&mut self) -> Result<(), String> {
        self.paused += 1;
        Ok(())
    }

    fn resume(&mut self) -> Result<(), String> {
        self.resumed += 1;
        Ok(())
    }

    fn set_rate(&mut self, rate: f32) -> Result<(), String> {
        self.rate = rate;
        Ok(())
    }

    fn set_pitch(&mut self, pitch: f32) -> Result<(), String> {
        self.pitch = pitch;
        Ok(())
    }

    fn poll(&mut self) -> Vec<HostEvent> {
        self.events.drain(..).collect()
    }
}

/// Maximum queued utterances (current + next prefetch, spec §4).
const QUEUE_DEPTH: usize = 2;

/// Errors before the engine gives up (spec §5 retry policy).
const MAX_CONSECUTIVE_ERRORS: u32 = 3;

/// Owns the narration queue and state machine.
#[derive(Debug)]
pub struct NarrationEngine {
    state: EngineState,
    chunks: Vec<NarrationChunk>,
    index: usize,
    queue: VecDeque<u64>,
    next_utterance_id: u64,
    consecutive_errors: u32,
    rate: f32,
    pitch: f32,
    error_message: Option<String>,
}

impl NarrationEngine {
    /// Create an idle engine with the given rate/pitch.
    pub fn new(rate: f32, pitch: f32) -> Self {
        Self {
            state: EngineState::Idle,
            chunks: Vec::new(),
            index: 0,
            queue: VecDeque::new(),
            next_utterance_id: 1,
            consecutive_errors: 0,
            rate,
            pitch,
            error_message: None,
        }
    }

    /// Current engine state.
    pub fn state(&self) -> EngineState {
        self.state
    }

    /// Rate in effect.
    pub fn rate(&self) -> f32 {
        self.rate
    }

    /// Pitch in effect.
    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    /// (current chunk index, total chunks) — 0-based.
    pub fn progress(&self) -> (usize, usize) {
        (self.index.min(self.chunks.len()), self.chunks.len())
    }

    /// The chunk currently being spoken, if any.
    pub fn current_chunk(&self) -> Option<&NarrationChunk> {
        self.chunks.get(self.index)
    }

    /// Replace the chunk list and enter `Loading`.
    pub fn load_chunks(&mut self, chunks: Vec<NarrationChunk>) {
        self.chunks = chunks;
        self.index = 0;
        self.queue.clear();
        self.consecutive_errors = 0;
        self.error_message = None;
        if !self.chunks.is_empty() {
            self.state = EngineState::Loading;
        }
    }

    /// Begin speaking from the current chunk (Loading or Idle → Speaking).
    pub fn start(&mut self, host: &mut dyn TtsHost) -> Result<(), String> {
        if self.chunks.is_empty() {
            return Err("no chunks loaded".to_string());
        }
        host.set_rate(self.rate)?;
        host.set_pitch(self.pitch)?;
        self.state = EngineState::Speaking;
        while self.queue.len() < QUEUE_DEPTH {
            let Some(chunk) = self.chunks.get(self.index + self.queue.len()) else {
                break;
            };
            let id = self.next_utterance_id;
            self.next_utterance_id += 1;
            host.speak(id, &chunk.text)?;
            self.queue.push_back(id);
        }
        Ok(())
    }

    /// Pause (Speaking → Paused).
    pub fn pause(&mut self, host: &mut dyn TtsHost) {
        if self.state != EngineState::Speaking {
            return;
        }
        let _ = host.pause();
        self.state = EngineState::Paused;
    }

    /// Resume (Paused → Speaking).
    pub fn resume(&mut self, host: &mut dyn TtsHost) {
        if self.state != EngineState::Paused {
            return;
        }
        let _ = host.resume();
        self.state = EngineState::Speaking;
    }

    /// Stop and clear (any state → Idle).
    pub fn stop(&mut self, host: &mut dyn TtsHost) {
        let _ = host.stop();
        self.queue.clear();
        self.consecutive_errors = 0;
        self.error_message = None;
        self.state = EngineState::Idle;
    }

    /// Adjust the speech rate and propagate to the host.
    pub fn set_rate(&mut self, host: &mut dyn TtsHost, rate: f32) {
        self.rate = rate.clamp(0.5, 2.5);
        let _ = host.set_rate(self.rate);
    }

    /// Adjust the pitch and propagate to the host.
    pub fn set_pitch(&mut self, host: &mut dyn TtsHost, pitch: f32) {
        self.pitch = pitch.clamp(0.5, 1.5);
        let _ = host.set_pitch(self.pitch);
    }

    /// Consume pending host events and return effects.
    pub fn poll(&mut self, host: &mut dyn TtsHost) -> Vec<EngineEffect> {
        let mut effects = Vec::new();
        for event in host.poll() {
            if let Some(effect) = self.handle_event(host, event) {
                effects.push(effect);
            }
        }
        effects
    }

    fn handle_event(&mut self, host: &mut dyn TtsHost, event: HostEvent) -> Option<EngineEffect> {
        match event {
            HostEvent::Started { .. } => None,
            HostEvent::Range {
                utterance_id,
                start,
                end,
            } => {
                let chunk = self.chunks.get(self.index)?;
                if self.queue.front().copied() != Some(utterance_id) {
                    return None;
                }
                Some(EngineEffect::WordHighlight {
                    block_index: chunk.block_index,
                    char_start: chunk.char_start + start,
                    char_end: chunk.char_start + end,
                })
            }
            HostEvent::Done { utterance_id } => {
                self.queue.retain(|id| *id != utterance_id);
                self.consecutive_errors = 0;
                self.advance(host)
            }
            HostEvent::Error { utterance_id } => {
                self.queue.retain(|id| *id != utterance_id);
                self.consecutive_errors += 1;
                if self.consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    self.stop(host);
                    self.state = EngineState::Error;
                    let message = format!("TTS failed {} times in a row", MAX_CONSECUTIVE_ERRORS);
                    self.error_message = Some(message.clone());
                    return Some(EngineEffect::Error { message });
                }
                self.advance(host)
            }
            HostEvent::Control(action) => {
                match action {
                    ControlAction::Play => self.resume(host),
                    ControlAction::Pause => self.pause(host),
                    ControlAction::Stop => self.stop(host),
                    ControlAction::SkipBack => self.skip(host, -1),
                    ControlAction::SkipForward => self.skip(host, 1),
                    ControlAction::SpeedUp => self.set_rate(host, self.rate + 0.1),
                    ControlAction::SpeedDown => self.set_rate(host, self.rate - 0.1),
                }
                None
            }
        }
    }

    /// Move the narration cursor `delta` chunks and restart speech from
    /// there (notification skip-back / skip-forward, TTS_SPEC §4). No-op
    /// outside Speaking/Paused or at the ends of the chunk list.
    fn skip(&mut self, host: &mut dyn TtsHost, delta: isize) {
        if self.chunks.is_empty() {
            return;
        }
        if !matches!(self.state, EngineState::Speaking | EngineState::Paused) {
            return;
        }
        let target =
            (self.index as isize + delta).clamp(0, self.chunks.len() as isize - 1) as usize;
        if target == self.index {
            return;
        }
        self.index = target;
        self.queue.clear();
        self.consecutive_errors = 0;
        let _ = host.stop();
        let _ = self.start(host);
    }

    /// Move to the next chunk (or finish), refilling the prefetch queue.
    fn advance(&mut self, host: &mut dyn TtsHost) -> Option<EngineEffect> {
        while self.queue.len() < QUEUE_DEPTH {
            let next = self.index + self.queue.len() + 1;
            let Some(chunk) = self.chunks.get(next) else {
                break;
            };
            let id = self.next_utterance_id;
            self.next_utterance_id += 1;
            if host.speak(id, &chunk.text).is_err() {
                break;
            }
            self.queue.push_back(id);
        }
        if self.queue.is_empty() {
            self.state = EngineState::Idle;
            // Tear down the host side too (Android foreground service).
            let _ = host.stop();
            return Some(EngineEffect::Finished);
        }
        self.index += 1;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reeda_epub::cfi::Cfi;
    use reeda_epub::cfi::CfiRange;

    fn chunk(id: u32, text: &str) -> NarrationChunk {
        NarrationChunk {
            block_index: id,
            char_start: 0,
            char_end: text.len() as u32,
            text: text.to_string(),
            cfi: CfiRange {
                start: Cfi("epubcfi(/6/4!/4/2/2/1:0)".into()),
                end: Cfi("epubcfi(/6/4!/4/2/2/1:5)".into()),
            },
        }
    }

    #[test]
    fn start_queues_two_chunks_and_prefetches() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.load_chunks(vec![chunk(0, "one"), chunk(1, "two"), chunk(2, "three")]);
        engine.start(&mut host).unwrap();
        assert_eq!(engine.state(), EngineState::Speaking);
        assert_eq!(
            host.spoken()
                .iter()
                .map(|(id, t)| (*id, t.as_str()))
                .collect::<Vec<_>>(),
            vec![(1, "one"), (2, "two")]
        );
        // First chunk done → prefetch third.
        host.push_event(HostEvent::Done { utterance_id: 1 });
        let effects = engine.poll(&mut host);
        assert!(effects.is_empty());
        assert_eq!(
            host.spoken()
                .iter()
                .map(|(_, t)| t.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two", "three"]
        );
        assert_eq!(engine.progress(), (1, 3));
    }

    #[test]
    fn done_on_last_chunk_emits_finished() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.load_chunks(vec![chunk(0, "only")]);
        engine.start(&mut host).unwrap();
        let effects = engine.poll(&mut {
            let mut h = FakeTtsHost::new();
            h.push_event(HostEvent::Done { utterance_id: 1 });
            h
        });
        assert_eq!(effects, vec![EngineEffect::Finished]);
        assert_eq!(engine.state(), EngineState::Idle);
    }

    #[test]
    fn range_events_map_to_block_offsets() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        let mut c = chunk(7, "hello world");
        c.char_start = 100;
        engine.load_chunks(vec![c]);
        engine.start(&mut host).unwrap();
        let effects = engine.poll(&mut {
            let mut h = FakeTtsHost::new();
            h.push_event(HostEvent::Range {
                utterance_id: 1,
                start: 6,
                end: 11,
            });
            h
        });
        assert_eq!(
            effects,
            vec![EngineEffect::WordHighlight {
                block_index: 7,
                char_start: 106,
                char_end: 111
            }]
        );
    }

    #[test]
    fn pause_resume_stop_transitions() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.load_chunks(vec![chunk(0, "one"), chunk(1, "two")]);
        engine.start(&mut host).unwrap();

        engine.pause(&mut host);
        assert_eq!(engine.state(), EngineState::Paused);
        assert_eq!(host.pause_count(), 1);

        engine.resume(&mut host);
        assert_eq!(engine.state(), EngineState::Speaking);
        assert_eq!(host.resume_count(), 1);

        engine.stop(&mut host);
        assert_eq!(engine.state(), EngineState::Idle);
        assert_eq!(host.stop_count(), 1);
        assert_eq!(engine.progress(), (0, 2));
    }

    #[test]
    fn three_consecutive_errors_pause_engine() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.load_chunks(vec![chunk(0, "one"), chunk(1, "two"), chunk(2, "three")]);
        engine.start(&mut host).unwrap();

        for id in 1..=3u64 {
            host.push_event(HostEvent::Error { utterance_id: id });
            let effects = engine.poll(&mut host);
            if id < 3 {
                assert!(effects.is_empty(), "should skip chunk {id}");
            } else {
                assert!(matches!(effects[0], EngineEffect::Error { .. }));
            }
        }
        assert_eq!(engine.state(), EngineState::Error);
        assert_eq!(host.stop_count(), 1);
    }

    #[test]
    fn rate_and_pitch_propagate() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.set_rate(&mut host, 2.0);
        engine.set_pitch(&mut host, 1.25);
        assert_eq!(host.rate(), 2.0);
        assert_eq!(host.pitch(), 1.25);
        engine.set_rate(&mut host, 99.0); // clamped
        assert_eq!(host.rate(), 2.5);
    }

    #[test]
    fn start_without_chunks_errors() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        assert!(engine.start(&mut host).is_err());
        assert_eq!(engine.state(), EngineState::Idle);
    }

    #[test]
    fn unknown_utterance_events_are_ignored() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.load_chunks(vec![chunk(0, "one")]);
        engine.start(&mut host).unwrap();
        host.push_event(HostEvent::Done { utterance_id: 42 });
        host.push_event(HostEvent::Range {
            utterance_id: 42,
            start: 0,
            end: 3,
        });
        let effects = engine.poll(&mut host);
        assert!(effects.is_empty());
        assert_eq!(engine.state(), EngineState::Speaking);
    }

    #[test]
    fn control_pause_resume_play_map_to_host_calls() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.load_chunks(vec![chunk(0, "one")]);
        engine.start(&mut host).unwrap();

        host.push_event(HostEvent::Control(ControlAction::Pause));
        assert!(engine.poll(&mut host).is_empty());
        assert_eq!(engine.state(), EngineState::Paused);
        assert_eq!(host.pause_count(), 1);

        host.push_event(HostEvent::Control(ControlAction::Play));
        assert!(engine.poll(&mut host).is_empty());
        assert_eq!(engine.state(), EngineState::Speaking);
        assert_eq!(host.resume_count(), 1);
    }

    #[test]
    fn control_stop_tears_down() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.load_chunks(vec![chunk(0, "one")]);
        engine.start(&mut host).unwrap();

        host.push_event(HostEvent::Control(ControlAction::Stop));
        assert!(engine.poll(&mut host).is_empty());
        assert_eq!(engine.state(), EngineState::Idle);
        assert_eq!(host.stop_count(), 1);
    }

    #[test]
    fn control_skip_forward_moves_to_next_chunk() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.load_chunks(vec![chunk(0, "one"), chunk(1, "two"), chunk(2, "three")]);
        engine.start(&mut host).unwrap();

        host.push_event(HostEvent::Control(ControlAction::SkipForward));
        assert!(engine.poll(&mut host).is_empty());
        assert_eq!(engine.progress(), (1, 3));
        assert_eq!(engine.state(), EngineState::Speaking);
        // The queue refills from the skipped index (chunk 1 spoken again,
        // chunk 2 prefetched).
        let last = host.spoken().last().map(|(_, t)| t.as_str());
        assert_eq!(last, Some("three"));

        // Skipping past the last chunk clamps to it.
        host.push_event(HostEvent::Control(ControlAction::SkipForward));
        host.push_event(HostEvent::Control(ControlAction::SkipForward));
        assert!(engine.poll(&mut host).is_empty());
        assert_eq!(engine.progress(), (2, 3));
    }

    #[test]
    fn control_skip_back_moves_to_previous_chunk() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.load_chunks(vec![chunk(0, "one"), chunk(1, "two")]);
        engine.start(&mut host).unwrap();

        host.push_event(HostEvent::Control(ControlAction::SkipBack));
        assert!(engine.poll(&mut host).is_empty());
        assert_eq!(
            engine.progress(),
            (0, 2),
            "skip back at first chunk is a no-op"
        );

        host.push_event(HostEvent::Done { utterance_id: 1 });
        engine.poll(&mut host);
        assert_eq!(engine.progress(), (1, 2));

        host.push_event(HostEvent::Control(ControlAction::SkipBack));
        assert!(engine.poll(&mut host).is_empty());
        assert_eq!(engine.progress(), (0, 2));
        let last = host.spoken().last().map(|(_, t)| t.as_str());
        assert_eq!(
            last,
            Some("two"),
            "queue refills from the skipped-back index"
        );
    }

    #[test]
    fn control_speed_steps_rate_by_0_1_within_bounds() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        engine.load_chunks(vec![chunk(0, "one")]);
        engine.start(&mut host).unwrap();

        for _ in 0..20 {
            host.push_event(HostEvent::Control(ControlAction::SpeedUp));
        }
        assert!(engine.poll(&mut host).is_empty());
        assert_eq!(engine.rate(), 2.5);

        for _ in 0..30 {
            host.push_event(HostEvent::Control(ControlAction::SpeedDown));
        }
        assert!(engine.poll(&mut host).is_empty());
        assert_eq!(engine.rate(), 0.5);
        assert_eq!(host.rate(), 0.5);
    }

    #[test]
    fn control_in_idle_state_is_ignored() {
        let mut host = FakeTtsHost::new();
        let mut engine = NarrationEngine::new(1.0, 1.0);
        for action in [
            ControlAction::Play,
            ControlAction::Pause,
            ControlAction::SkipForward,
            ControlAction::SkipBack,
        ] {
            host.push_event(HostEvent::Control(action));
        }
        assert!(engine.poll(&mut host).is_empty());
        assert_eq!(engine.state(), EngineState::Idle);
        assert_eq!(host.stop_count(), 0);
    }
}
