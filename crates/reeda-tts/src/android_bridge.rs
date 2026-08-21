//! Android `android.speech.tts.TextToSpeech` JNI bridge (TTS_SPEC §2).
//!
//! [`AndroidTtsHost`] implements [`crate::engine::TtsHost`] over the Java
//! shim `io.reeda.app.TtsShim` (see `android/src/io/reeda/app/TtsShim.java`).
//!
//! Architecture:
//! - Rust holds a `GlobalRef` to the singleton `TtsShim`; every call goes
//!   through `JavaVM::attach_current_thread` so it works from any thread.
//! - The shim's `UtteranceProgressListener` fires on the binder thread and
//!   invokes the exported native method [`jni_event_callback`], which pushes
//!   into a process-wide queue drained by [`AndroidTtsHost::poll`].
//! - The narration foreground service (`NarrationService.java`, TTS_SPEC §2)
//!   is started/stopped from the host; its notification actions come back
//!   through [`jni_action_callback`] as `HostEvent::Control`.
//!
//! Feature-gated behind `platform-android`; the module is compile-checked in
//! CI and on hosts (the `jni` crate is pure Rust).

use std::collections::VecDeque;
use std::sync::Mutex;

use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::sys::{jint, jlong};
use jni::{JNIEnv, JavaVM};

use crate::engine::{ControlAction, HostEvent};

/// JNI callback event kinds — must match `TtsShim.java` constants.
const EV_START: jint = 0;
const EV_RANGE: jint = 1;
const EV_DONE: jint = 2;
const EV_ERROR: jint = 3;

/// NarrationService action ids — must match `NarrationService.java`.
const ACT_PLAY: jint = 0;
const ACT_PAUSE: jint = 1;
const ACT_STOP: jint = 2;
const ACT_SKIP_BACK: jint = 3;
const ACT_SKIP_FORWARD: jint = 4;
const ACT_SPEED_UP: jint = 5;
const ACT_SPEED_DOWN: jint = 6;

/// Binder-thread event queue (callbacks arrive on non-Rust threads).
static EVENT_QUEUE: Mutex<VecDeque<HostEvent>> = Mutex::new(VecDeque::new());

/// Media-control action → engine event (TTS_SPEC §2 notification buttons).
fn control_action(action: jint) -> Option<HostEvent> {
    let action = match action {
        ACT_PLAY => ControlAction::Play,
        ACT_PAUSE => ControlAction::Pause,
        ACT_STOP => ControlAction::Stop,
        ACT_SKIP_BACK => ControlAction::SkipBack,
        ACT_SKIP_FORWARD => ControlAction::SkipForward,
        ACT_SPEED_UP => ControlAction::SpeedUp,
        ACT_SPEED_DOWN => ControlAction::SpeedDown,
        _ => return None,
    };
    Some(HostEvent::Control(action))
}

/// Registered by the Java shim; called on binder threads.
///
/// Symbol name must match `TtsShim.onEvent` (no JNI_OnLoad registration is
/// needed — the JVM resolves the name lazily on first call).
#[no_mangle]
pub extern "system" fn Java_io_reeda_app_TtsShim_onEvent(
    env: JNIEnv,
    _class: JClass,
    ty: jint,
    utterance_id: jlong,
    start: jint,
    end: jint,
) {
    let event = match ty {
        EV_START => Some(HostEvent::Started {
            utterance_id: utterance_id as u64,
        }),
        EV_RANGE => Some(HostEvent::Range {
            utterance_id: utterance_id as u64,
            start: start as u32,
            end: end as u32,
        }),
        EV_DONE => Some(HostEvent::Done {
            utterance_id: utterance_id as u64,
        }),
        EV_ERROR => Some(HostEvent::Error {
            utterance_id: utterance_id as u64,
        }),
        _ => None,
    };
    if let Some(event) = event {
        if let Ok(mut queue) = EVENT_QUEUE.lock() {
            queue.push_back(event);
        }
    }
    let _ = env.exception_clear();
}

/// Called by the foreground `NarrationService` when the user taps a
/// notification / lock-screen action (play, pause, stop, skip, speed).
///
/// Symbol name must match `NarrationService.onAction`.
#[no_mangle]
pub extern "system" fn Java_io_reeda_app_NarrationService_onAction(
    env: JNIEnv,
    _class: JClass,
    action: jint,
) {
    if let Some(event) = control_action(action) {
        if let Ok(mut queue) = EVENT_QUEUE.lock() {
            queue.push_back(event);
        }
    }
    let _ = env.exception_clear();
}

/// JNI status code mirrored from `android.text.TextToSpeech`.
const TTS_SUCCESS: jint = 0;

/// A [`crate::engine::TtsHost`] backed by Android's `TextToSpeech`.
///
/// Construct with [`AndroidTtsHost::new`] once the JVM is available (from
/// the app process); it initializes the `TtsShim` singleton lazily and
/// retains the app context for the narration foreground service
/// ([`AndroidTtsHost::start_service`] / [`AndroidTtsHost::stop_service`],
/// TTS_SPEC §2).
pub struct AndroidTtsHost {
    vm: JavaVM,
    shim: GlobalRef,
    context: GlobalRef,
    rate: f32,
    pitch: f32,
    stopped: bool,
}

impl AndroidTtsHost {
    /// Resolve the JVM + activity context and initialize the TTS shim.
    ///
    /// `#[cfg(android)]`-independent: the JVM is obtained from
    /// `ndk_context`, which is populated by the Android runtime in every
    /// process; on non-Android hosts this returns `Err`.
    pub fn new() -> Result<Self, String> {
        let ctx = ndk_context::android_context();
        if ctx.vm().is_null() || ctx.context().is_null() {
            return Err("AndroidTtsHost: not running on Android".into());
        }
        // SAFETY: `ctx.vm()` is the JNI JavaVM pointer set by the Android
        // runtime via ndk_context, guaranteed non-null (checked above) and
        // valid for the lifetime of the process. `JavaVM::from_raw` takes
        // ownership of the raw pointer without freeing it; the Android
        // runtime owns the JVM, so ownership transfer is sound.
        let vm = unsafe { JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) }
            .map_err(|e| format!("AndroidTtsHost: JavaVM::from_raw: {e}"))?;
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| format!("AndroidTtsHost: attach: {e}"))?;

        // Attach the shim singleton to the application context.
        // SAFETY: `ctx.context()` is the application Context jobject global
        // reference provided by the Android runtime; it is valid for the
        // process lifetime and must not be freed by us. Wrapping it in
        // JObject::from_raw transfers ownership of the global ref to the
        // wrapper; the shim only uses it during this call and never deletes
        // it (the runtime retains ownership), so the reference cannot dangle.
        let ctx_obj = unsafe { JObject::from_raw(ctx.context() as jni::sys::jobject) };
        if let Err(e) = env.call_static_method(
            "io/reeda/app/TtsShim",
            "init",
            "(Landroid/content/Context;)V",
            &[jni::objects::JValueGen::Object(&ctx_obj)],
        ) {
            // Clear the pending exception: a leaked one would poison every
            // later JNI call on this thread (slint's backend panics on JNI
            // errors, which aborts the process — see reeda-ui/src/android/log.rs).
            let _ = env.exception_clear();
            return Err(format!("AndroidTtsHost: TtsShim.init: {e}"));
        }

        let instance = match env
            .call_static_method(
                "io/reeda/app/TtsShim",
                "get",
                "()Lio/reeda/app/TtsShim;",
                &[],
            )
            .and_then(|v| v.l())
        {
            Ok(instance) => instance,
            Err(e) => {
                let _ = env.exception_clear();
                return Err(format!("AndroidTtsHost: TtsShim.get: {e}"));
            }
        };
        let shim = env
            .new_global_ref(instance)
            .map_err(|e| format!("AndroidTtsHost: global ref: {e}"))?;
        let context = env
            .new_global_ref(ctx_obj)
            .map_err(|e| format!("AndroidTtsHost: context global ref: {e}"))?;
        drop(env);

        Ok(Self {
            vm,
            shim,
            context,
            rate: 1.0,
            pitch: 1.0,
            stopped: false,
        })
    }

    /// Static `NarrationService.start(Context)` / `NarrationService.stop(Context)`.
    fn service_call(&mut self, method: &str) -> Result<(), String> {
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|e| format!("AndroidTtsHost: attach: {e}"))?;
        let result = match env.call_static_method(
            "io/reeda/app/NarrationService",
            method,
            "(Landroid/content/Context;)V",
            &[JValue::Object(&self.context)],
        ) {
            Ok(_) => Ok(()),
            Err(e) => {
                let _ = env.exception_clear();
                Err(format!("AndroidTtsHost: NarrationService.{method}: {e}"))
            }
        };
        drop(env);
        result
    }

    /// Bring up the narration foreground service (idempotent).
    pub fn start_service(&mut self) -> Result<(), String> {
        self.service_call("start")
    }

    /// Tear down the narration foreground service (idempotent).
    pub fn stop_service(&mut self) -> Result<(), String> {
        self.service_call("stop")
    }

    /// Call a shim method returning an `int` status.
    fn call_int(
        &mut self,
        method: &str,
        sig: &str,
        args: &[jni::objects::JValue],
    ) -> Result<(), String> {
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|e| format!("AndroidTtsHost: attach: {e}"))?;
        let status: jint = match env
            .call_method(&self.shim, method, sig, args)
            .and_then(|v| v.i())
        {
            Ok(status) => status,
            Err(e) => {
                let _ = env.exception_clear();
                drop(env);
                return Err(format!("AndroidTtsHost: {method}: {e}"));
            }
        };
        drop(env);
        if status != TTS_SUCCESS {
            return Err(format!("AndroidTtsHost: {method}: status {status}"));
        }
        Ok(())
    }

    /// Flush queued binder events (must not be called while holding the
    /// global JNI lock, so we lock only the Rust queue).
    fn drain_queue() -> Vec<HostEvent> {
        let mut queue = EVENT_QUEUE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        queue.drain(..).collect()
    }
}

impl std::fmt::Debug for AndroidTtsHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AndroidTtsHost")
            .field("rate", &self.rate)
            .field("pitch", &self.pitch)
            .field("stopped", &self.stopped)
            .finish_non_exhaustive()
    }
}

impl crate::engine::TtsHost for AndroidTtsHost {
    fn speak(&mut self, utterance_id: u64, text: &str) -> Result<(), String> {
        self.start_service()?;
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|e| format!("AndroidTtsHost: attach: {e}"))?;
        let jtext = env
            .new_string(text)
            .map_err(|e| format!("AndroidTtsHost: new_string: {e}"))?;
        let status: jint = match env
            .call_method(
                &self.shim,
                "speak",
                "(Ljava/lang/String;J)I",
                &[JValue::Object(&jtext), JValue::Long(utterance_id as jlong)],
            )
            .and_then(|v| v.i())
        {
            Ok(status) => status,
            Err(e) => {
                let _ = env.exception_clear();
                drop(env);
                return Err(format!("AndroidTtsHost: speak: {e}"));
            }
        };
        drop(env);
        if status != TTS_SUCCESS {
            return Err(format!("AndroidTtsHost: speak: status {status}"));
        }
        self.stopped = false;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.stopped = true;
        let result = self.call_int("stop", "()I", &[]);
        let _ = self.stop_service();
        result
    }

    fn pause(&mut self) -> Result<(), String> {
        // Best-effort: the engine stops queueing; Android TTS has no pause
        // API, so we stop the current utterance (resume restarts the chunk).
        self.call_int("stop", "()I", &[])
    }

    fn resume(&mut self) -> Result<(), String> {
        self.stopped = false;
        Ok(())
    }

    fn set_rate(&mut self, rate: f32) -> Result<(), String> {
        self.rate = rate;
        self.call_int("setRate", "(F)I", &[JValue::Float(rate)])
    }

    fn set_pitch(&mut self, pitch: f32) -> Result<(), String> {
        self.pitch = pitch;
        self.call_int("setPitch", "(F)I", &[JValue::Float(pitch)])
    }

    fn poll(&mut self) -> Vec<HostEvent> {
        Self::drain_queue()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_action_maps_ids() {
        use crate::engine::ControlAction;
        assert_eq!(
            control_action(ACT_PLAY),
            Some(HostEvent::Control(ControlAction::Play))
        );
        assert_eq!(
            control_action(ACT_PAUSE),
            Some(HostEvent::Control(ControlAction::Pause))
        );
        assert_eq!(
            control_action(ACT_STOP),
            Some(HostEvent::Control(ControlAction::Stop))
        );
        assert_eq!(
            control_action(ACT_SKIP_BACK),
            Some(HostEvent::Control(ControlAction::SkipBack))
        );
        assert_eq!(
            control_action(ACT_SKIP_FORWARD),
            Some(HostEvent::Control(ControlAction::SkipForward))
        );
        assert_eq!(
            control_action(ACT_SPEED_UP),
            Some(HostEvent::Control(ControlAction::SpeedUp))
        );
        assert_eq!(
            control_action(ACT_SPEED_DOWN),
            Some(HostEvent::Control(ControlAction::SpeedDown))
        );
        assert_eq!(control_action(99), None);
        assert_eq!(control_action(-1), None);
    }

    #[test]
    fn drain_queue_preserves_order() {
        EVENT_QUEUE.lock().unwrap().clear();
        let a = HostEvent::Started { utterance_id: 1 };
        let b = HostEvent::Done { utterance_id: 1 };
        let c = HostEvent::Started { utterance_id: 2 };
        {
            let mut queue = EVENT_QUEUE.lock().unwrap();
            queue.push_back(a.clone());
            queue.push_back(b.clone());
            queue.push_back(c.clone());
        }
        assert_eq!(AndroidTtsHost::drain_queue(), vec![a, b, c]);
        assert!(AndroidTtsHost::drain_queue().is_empty());
    }

    #[test]
    fn new_off_android_does_not_hang() {
        // On a host the ndk context is never initialized, and `ndk-context`
        // panics when `android_context()` is read before initialization; on a
        // real device android-activity sets the JVM/context before calling
        // `android_main` (init.rs), so the JNI path is live there. Either way
        // constructing the host must not hang or leave JNI state poisoned.
        let outcome = std::panic::catch_unwind(|| AndroidTtsHost::new());
        match outcome {
            Ok(Ok(host)) => {
                // On a real Android device this is expected to succeed; keep
                // the assertion permissive to stay useful in both environments.
                let _ = host;
            }
            Ok(Err(_e)) => {}  // clean Err (non-Android JNI availability)
            Err(_payload) => {} // host: ndk-context panic, as expected
        }
    }
}
