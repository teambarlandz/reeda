//! Android UI bridge stubs for reeda-ui.
//!
//! These are thin wrappers that the Slint UI layer calls on Android.
//! They delegate to `reeda_core::platform::Platform` (which currently
//! returns stubs). Real JNI integration lands in M5.
//!
//! All items in this module are gated behind `#[cfg(feature = "platform-android")]`.

use reeda_core::platform::android::AndroidPlatform;
use reeda_core::platform::{Platform, PlatformResult};

/// Keep the JNI callbacks exported by `reeda-tts` in the final cdylib.
///
/// The JVM resolves `Java_io_reeda_app_*` symbols by name via `dlsym` on the
/// app library; nothing in Rust references them, so without `#[used]` the
/// linker would dead-code-eliminate them and every TTS/service call would
/// throw `UnsatisfiedLinkError`.
#[used]
#[cfg(feature = "platform-android")]
static KEEP_TTS_JNI: unsafe extern "system" fn(
    jni::JNIEnv,
    jni::objects::JClass,
    jni::sys::jint,
    jni::sys::jlong,
    jni::sys::jint,
    jni::sys::jint,
) = reeda_tts::android_bridge::Java_io_reeda_app_TtsShim_onEvent;

/// Same as [`KEEP_TTS_JNI`], for the NarrationService action callback.
#[used]
#[cfg(feature = "platform-android")]
static KEEP_SERVICE_JNI: unsafe extern "system" fn(
    jni::JNIEnv,
    jni::objects::JClass,
    jni::sys::jint,
) = reeda_tts::android_bridge::Java_io_reeda_app_NarrationService_onAction;

/// Absolute path to the app-private files directory
/// (`context.getFilesDir().getAbsolutePath()`).
///
/// All persistent data (books, db, index, covers) lives under this
/// directory on Android (docs/TECHNICAL_DESIGN.md). Falls back to a
/// relative path on hosts without the ndk context (tests).
pub fn data_dir() -> String {
    let ctx = ndk_context::android_context();
    if ctx.vm().is_null() || ctx.context().is_null() {
        return String::from("reeda_data");
    }
    // SAFETY: `ctx.vm()`/`ctx.context()` are the JVM/Context pointers set
    // by the Android runtime via ndk_context, valid for the process
    // lifetime; checked non-null above. `JavaVM::from_raw` takes ownership
    // of the raw pointer (which we never free) — the runtime owns the JVM.
    let vm = match unsafe { jni::JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) } {
        Ok(vm) => vm,
        Err(_) => return String::from("reeda_data"),
    };
    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(_) => return String::from("reeda_data"),
    };
    // SAFETY: `ctx.context()` is the app Context global reference owned by
    // the runtime; wrapped without freeing, valid for the call duration.
    let context = unsafe { jni::objects::JObject::from_raw(ctx.context() as jni::sys::jobject) };
    let file = match env
        .call_method(&context, "getFilesDir", "()Ljava/io/File;", &[])
        .and_then(|v| v.l())
    {
        Ok(file) => file,
        Err(_) => return String::from("reeda_data"),
    };
    let path = match env
        .call_method(&file, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
    {
        Ok(path) => path,
        Err(_) => return String::from("reeda_data"),
    };
    let js = jni::objects::JString::from(path);
    let s: String = match env.get_string(&js) {
        Ok(s) => s.into(),
        Err(_) => return String::from("reeda_data"),
    };
    drop(env);
    s
}

/// Pick a file via SAF (Storage Access Framework).
///
/// Opens `ACTION_OPEN_DOCUMENT` and returns the content URI on success.
#[allow(dead_code)] // wired into the import flow in the platform milestone
pub fn pick_file(mime_type: &str) -> PlatformResult<String> {
    AndroidPlatform::default().pick_file(mime_type)
}

/// Read the URI from an incoming intent (share / open-with).
///
/// Returns `None` if no intent data is available.
#[allow(dead_code)] // wired into the import flow in the platform milestone
pub fn get_intent_data() -> PlatformResult<Option<String>> {
    AndroidPlatform::default().get_intent_data()
}

/// Request a runtime permission from the user.
///
/// Returns `true` if granted, `false` if denied.
#[allow(dead_code)] // wired into the import flow in the platform milestone
pub fn request_permission(permission: &str) -> PlatformResult<bool> {
    AndroidPlatform::default().request_permission(permission)
}

/// Create the JNI-backed TTS host for the narration engine.
///
/// Initializes `io.reeda.app.TtsShim` on the current thread and returns a
/// host whose callbacks are drained by the engine's `PollNarration` loop.
/// Called once at startup on `platform-android` builds.
pub fn create_tts_host() -> Result<Box<dyn reeda_tts::engine::TtsHost>, String> {
    reeda_tts::android_bridge::AndroidTtsHost::new()
        .map(|host| Box::new(host) as Box<dyn reeda_tts::engine::TtsHost>)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_file_returns_not_supported() {
        assert!(pick_file("application/epub+zip").is_err());
    }

    #[test]
    fn get_intent_data_returns_none() {
        assert!(get_intent_data().unwrap().is_none());
    }

    #[test]
    fn request_permission_returns_true() {
        assert!(request_permission("android.permission.READ_EXTERNAL_STORAGE").unwrap());
    }
}
