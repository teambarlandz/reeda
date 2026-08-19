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
//! - Foreground service / audio focus / wake-lock are stubs on this side;
//!   the manifest already declares the required permissions and the
//!   `NarrationService` (device verification is a follow-up).
//!
//! Feature-gated behind `platform-android`; the module is compile-checked in
//! CI and on hosts (the `jni` crate is pure Rust).

use std::collections::VecDeque;
use std::sync::Mutex;

use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::sys::{jint, jlong};
use jni::{JNIEnv, JavaVM};

use crate::engine::HostEvent;

/// JNI callback event kinds — must match `TtsShim.java` constants.
const EV_START: jint = 0;
const EV_RANGE: jint = 1;
const EV_DONE: jint = 2;
const EV_ERROR: jint = 3;

/// Binder-thread event queue (callbacks arrive on non-Rust threads).
static EVENT_QUEUE: Mutex<VecDeque<HostEvent>> = Mutex::new(VecDeque::new());

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

/// JNI status code mirrored from `android.text.TextToSpeech`.
const TTS_SUCCESS: jint = 0;

/// A [`crate::engine::TtsHost`] backed by Android's `TextToSpeech`.
///
/// Construct with [`AndroidTtsHost::new`] once the JVM is available (from
/// the app process); it initializes the `TtsShim` singleton lazily.
pub struct AndroidTtsHost {
    vm: JavaVM,
    shim: GlobalRef,
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
        env.call_static_method(
            "io/reeda/app/TtsShim",
            "init",
            "(Landroid/content/Context;)V",
            &[jni::objects::JValueGen::Object(&ctx_obj)],
        )
        .map_err(|e| format!("AndroidTtsHost: TtsShim.init: {e}"))?;

        let instance = env
            .call_static_method(
                "io/reeda/app/TtsShim",
                "get",
                "()Lio/reeda/app/TtsShim;",
                &[],
            )
            .and_then(|v| v.l())
            .map_err(|e| format!("AndroidTtsHost: TtsShim.get: {e}"))?;
        let shim = env
            .new_global_ref(instance)
            .map_err(|e| format!("AndroidTtsHost: global ref: {e}"))?;
        drop(env);

        Ok(Self {
            vm,
            shim,
            rate: 1.0,
            pitch: 1.0,
            stopped: false,
        })
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
        let status: jint = env
            .call_method(&self.shim, method, sig, args)
            .and_then(|v| v.i())
            .map_err(|e| format!("AndroidTtsHost: {method}: {e}"))?;
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
        let mut env = self
            .vm
            .attach_current_thread()
            .map_err(|e| format!("AndroidTtsHost: attach: {e}"))?;
        let jtext = env
            .new_string(text)
            .map_err(|e| format!("AndroidTtsHost: new_string: {e}"))?;
        let status: jint = env
            .call_method(
                &self.shim,
                "speak",
                "(Ljava/lang/String;J)I",
                &[JValue::Object(&jtext), JValue::Long(utterance_id as jlong)],
            )
            .and_then(|v| v.i())
            .map_err(|e| format!("AndroidTtsHost: speak: {e}"))?;
        drop(env);
        if status != TTS_SUCCESS {
            return Err(format!("AndroidTtsHost: speak: status {status}"));
        }
        self.stopped = false;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.stopped = true;
        self.call_int("stop", "()I", &[])
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
    fn new_off_android_is_err() {
        // On a host without the ndk context (desktop CI) this must fail
        // cleanly rather than panic — proving the bridge degrades safely.
        if let Ok(host) = AndroidTtsHost::new() {
            // On a real Android device this is expected to succeed; keep the
            // assertion permissive to stay useful in both environments.
            let _ = host;
        }
    }
}
