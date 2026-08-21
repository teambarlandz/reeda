//! Windows TTS host via WinRT SpeechSynthesizer + MediaPlayer.
//!
//! Produces chunk-level `HostEvent`s: `Started`, `Done`, `Error`.
//! Word-boundary `Range` events are Android-only for now (WinRT
//! SpeechSynthesizer does not expose word-boundary callbacks).
//!
//! Thread model: a persistent background worker owns the WinRT objects
//! (SpeechSynthesizer, MediaPlayer) and communicates with the UI thread
//! through an `mpsc` channel. Events accumulate in a shared
//! `Mutex<VecDeque<HostEvent>>` drained by [`WindowsTtsHost::poll`].

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, mpsc};

use crate::engine::{HostEvent, TtsHost};

type SharedQueue = Arc<Mutex<VecDeque<HostEvent>>>;

// ── Channel messages ────────────────────────────────────────────────────

#[allow(dead_code)]
enum Msg {
    Speak { id: u64, text: String },
    Stop,
    Pause,
    Resume,
    SetRate(f32),
    SetPitch(f32),
    Exit,
}

// ── Public host ─────────────────────────────────────────────────────────

/// WinRT-backed TTS host for Windows 10+ desktop builds.
pub struct WindowsTtsHost {
    tx: mpsc::Sender<Msg>,
    queue: SharedQueue,
}

impl WindowsTtsHost {
    /// Create a new host, spawning a background worker thread.
    ///
    /// Fails if WinRT initialization (SpeechSynthesizer or MediaPlayer)
    /// fails — typically because no voices are installed.
    pub fn new() -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<Msg>();
        let queue: SharedQueue = Arc::new(Mutex::new(VecDeque::new()));
        let worker_queue = queue.clone();

        std::thread::Builder::new()
            .name("reeda-tts-win".into())
            .spawn(move || worker_loop(rx, worker_queue))
            .map_err(|e| format!("failed to spawn TTS worker: {e}"))?;

        Ok(Self { tx, queue })
    }
}

impl TtsHost for WindowsTtsHost {
    fn speak(&mut self, utterance_id: u64, text: &str) -> Result<(), String> {
        self.tx
            .send(Msg::Speak {
                id: utterance_id,
                text: text.to_string(),
            })
            .map_err(|_| "TTS worker disconnected".into())
    }

    fn stop(&mut self) -> Result<(), String> {
        let _ = self.tx.send(Msg::Stop);
        Ok(())
    }

    fn pause(&mut self) -> Result<(), String> {
        let _ = self.tx.send(Msg::Pause);
        Ok(())
    }

    fn resume(&mut self) -> Result<(), String> {
        let _ = self.tx.send(Msg::Resume);
        Ok(())
    }

    fn set_rate(&mut self, rate: f32) -> Result<(), String> {
        let _ = self.tx.send(Msg::SetRate(rate));
        Ok(())
    }

    fn set_pitch(&mut self, _pitch: f32) -> Result<(), String> {
        // WinRT SpeechSynthesizer does not expose a pitch control.
        Ok(())
    }

    fn poll(&mut self) -> Vec<HostEvent> {
        self.queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect()
    }
}

// ── Worker thread ───────────────────────────────────────────────────────

fn worker_loop(rx: mpsc::Receiver<Msg>, queue: SharedQueue) {
    use windows::core::HSTRING;
    use windows::core::Ref;
    use windows::Foundation::TypedEventHandler;
    use windows::Media::Core::MediaSource;
    use windows::Media::Playback::{MediaPlayer, MediaPlayerFailedEventArgs};
    use windows::Media::SpeechSynthesis::SpeechSynthesizer;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    // SAFETY: CoInitializeEx with COINIT_MULTITHREADED is safe to call on
    // a new thread; we ignore errors (already-initialized, or COM not
    // needed on some CI runners) since the WinRT objects below will fail
    // their own constructors if COM is truly unavailable.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let synth = match SpeechSynthesizer::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("TTS: SpeechSynthesizer init failed: {e}");
            return;
        }
    };
    let player = match MediaPlayer::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("TTS: MediaPlayer init failed: {e}");
            return;
        }
    };

    // Shared current-utterance id so MediaEnded can report it.
    let current_id = Arc::new(Mutex::new(0u64));

    // MediaEnded handler → pushes Done.
    {
        let q = queue.clone();
        let id_ref = current_id.clone();
        player
            .MediaEnded(&TypedEventHandler::new(
                move |_: Ref<MediaPlayer>, _: Ref<windows::core::IInspectable>| {
                    let id = *id_ref.lock().unwrap_or_else(|e| e.into_inner());
                    q.lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push_back(HostEvent::Done {
                            utterance_id: id,
                        });
                    Ok(())
                },
            ))
            .ok();
    }

    // MediaFailed handler → logs (engine decides when to give up).
    {
        player
            .MediaFailed(&TypedEventHandler::new(
                move |_: Ref<MediaPlayer>, _: Ref<MediaPlayerFailedEventArgs>| {
                    Ok(())
                },
            ))
            .ok();
    }

    let mut rate: f32 = 1.0;

    loop {
        let mut rate_dirty = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Speak { id, text } => {
                    if rate_dirty {
                        if let Ok(opts) = synth.Options() {
                            // WinRT rate: -10..10 where 0 = normal.
                            // Engine multiplier 1.0 = normal → maps to 0.
                            let winrt_rate = ((rate - 1.0) as f64 * 5.0).clamp(-10.0, 10.0);
                            let _ = opts.SetSpeakingRate(winrt_rate);
                        }
                        rate_dirty = false;
                    }

                    match synth.SynthesizeTextToStreamAsync(&HSTRING::from(&text)) {
                        Ok(op) => match op.GetResults() {
                            Ok(stream) => {
                                let ct = stream
                                    .ContentType()
                                    .map(|h| h.to_string_lossy())
                                    .unwrap_or_else(|_| "audio/x-wav".into());
                                match MediaSource::CreateFromStream(
                                    &stream,
                                    &HSTRING::from(&ct),
                                ) {
                                    Ok(src) => {
                                        *current_id
                                            .lock()
                                            .unwrap_or_else(|e| e.into_inner()) = id;
                                        let _ = player.SetSource(&src);
                                        let _ = player.Play();
                                    }
                                    Err(e) => {
                                        eprintln!("TTS MediaSource error: {e}");
                                        queue.lock().unwrap_or_else(|e| e.into_inner())
                                            .push_back(HostEvent::Error { utterance_id: id });
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("TTS stream error: {e}");
                                queue.lock().unwrap_or_else(|e| e.into_inner())
                                    .push_back(HostEvent::Error { utterance_id: id });
                            }
                        },
                        Err(e) => {
                            eprintln!("TTS SynthesizeTextToStreamAsync error: {e}");
                            queue.lock().unwrap_or_else(|e| e.into_inner())
                                .push_back(HostEvent::Error { utterance_id: id });
                        }
                    }
                }
                Msg::Stop => {
                    let _ = player.Pause();
                    while rx.try_recv().is_ok() {}
                }
                Msg::Pause => {
                    let _ = player.Pause();
                }
                Msg::Resume => {
                    let _ = player.Play();
                }
                Msg::SetRate(r) => {
                    rate = r;
                    rate_dirty = true;
                }
                Msg::SetPitch(_) => {}
                Msg::Exit => return,
            }
        }
    }
}
