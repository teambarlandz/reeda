//! Startup event log (breadcrumbs) for diagnosing device crashes without adb.
//!
//! Every startup step is appended to `reeda-crash.log.txt` in:
//! 1. the app-private files dir (`getFilesDir()`),
//! 2. the app-external files dir (`getExternalFilesDir(null)`, no permission),
//! and the same content is mirrored to the public **Downloads** folder via
//! `io.reeda.app.LogExporter` so the user can share the file from any file
//! manager without USB or cloud access.
//!
//! A panic hook writes the panic message into the log before the process
//! aborts. The release profile uses `panic = "abort"`, but the hook still
//! runs first, so even a hard startup failure leaves "died at step N" behind.
//!
//! All writes are open-append-close (no shared file handle), so the panic
//! hook can write safely from any thread without lock poisoning concerns.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use jni::objects::{JObject, JString, JValue};
use jni::sys::jobject;
use jni::JavaVM;

/// Log file name (also used for the Downloads copy).
const FILE_NAME: &str = "reeda-crash.log.txt";

/// Resolved log destinations (populated once at startup).
struct LogPaths {
    internal: PathBuf,
    external: Option<PathBuf>,
}

static PATHS: OnceLock<LogPaths> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();

/// Initialize the event log: resolve the storage paths, write the header,
/// mirror it to Downloads, and install the panic hook. Safe to call once at
/// `android_main` entry; no-ops if the Android context is unavailable.
pub fn init() {
    let Some(paths) = resolve_paths() else {
        eprintln!("event log unavailable: no Android context");
        return;
    };
    let _ = PATHS.set(paths);
    let mut lines = vec![
        format!("device: {}", device_header()),
        "=== Reeda event log (one line per startup step) ===".to_string(),
    ];
    lines.push("log started".to_string());
    write_lines(PATHS.get().unwrap(), &lines);
    export();

    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {info}");
        if let Some(paths) = PATHS.get() {
            // Files first (pure fs, cannot fail due to JNI state); then
            // best-effort Downloads mirror (may fail if the panic happened
            // inside a JNI call — the external copy still survives).
            write_lines(paths, &[msg]);
            export();
        }
    }));
}

/// Append a breadcrumb line (timestamped) and refresh the Downloads copy.
pub fn trace(msg: &str) {
    if let Some(paths) = PATHS.get() {
        write_lines(paths, &[msg.to_string()]);
        export();
    }
}

/// `"manufacturer model (sdk N)"` — cheap device fingerprint for the log.
fn device_header() -> String {
    let ctx = ndk_context::android_context();
    if ctx.vm().is_null() || ctx.context().is_null() {
        return "unknown".to_string();
    }
    // SAFETY: `ctx.vm()` is the JVM pointer set by the Android runtime via
    // ndk_context, non-null (checked above) and valid for the process
    // lifetime; `JavaVM::from_raw` wraps it without freeing (the runtime
    // owns the JVM).
    let vm = match unsafe { JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) } {
        Ok(vm) => vm,
        Err(_) => return "unknown".to_string(),
    };
    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(_) => return "unknown".to_string(),
    };
    let model = match env
        .get_static_field("android/os/Build", "MODEL", "Ljava/lang/String;")
        .and_then(|v| v.l())
    {
        Ok(m) => env
            .get_string(&JString::from(m))
            .map(|s| s.into())
            .unwrap_or_else(|_| "?".to_string()),
        Err(_) => {
            let _ = env.exception_clear();
            "?".to_string()
        }
    };
    let sdk: jni::sys::jint = match env
        .get_static_field("android/os/Build$VERSION", "SDK_INT", "I")
        .and_then(|v| v.i())
    {
        Ok(i) => i,
        Err(_) => {
            let _ = env.exception_clear();
            -1
        }
    };
    drop(env);
    format!("{model} (sdk {sdk})")
}

/// Resolve the app-private + app-external log directories.
fn resolve_paths() -> Option<LogPaths> {
    let ctx = ndk_context::android_context();
    if ctx.vm().is_null() || ctx.context().is_null() {
        return None;
    }
    // SAFETY: `ctx.vm()`/`ctx.context()` are the JVM/Context pointers set by
    // the Android runtime via ndk_context, valid for the process lifetime;
    // checked non-null above. `JavaVM::from_raw` takes ownership of the raw
    // pointer (which we never free) — the runtime owns the JVM.
    let vm = unsafe { JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    // SAFETY: `ctx.context()` is the app Context global reference owned by
    // the runtime; wrapped without freeing, valid for the call duration.
    let context = unsafe { JObject::from_raw(ctx.context() as jobject) };

    let internal = path_from_method(&mut env, &context, "getFilesDir", "()Ljava/io/File;", &[])?;
    let external = path_from_method(
        &mut env,
        &context,
        "getExternalFilesDir",
        "(Ljava/lang/String;)Ljava/io/File;",
        &[JValue::Object(&JObject::null())],
    );
    drop(env);
    Some(LogPaths {
        internal: PathBuf::from(internal).join(FILE_NAME),
        external: external.map(|p| PathBuf::from(p).join(FILE_NAME)),
    })
}

/// Call a `Context` method returning a `File`, then `getAbsolutePath()`,
/// clearing any pending JNI exception on the way out.
fn path_from_method(
    env: &mut jni::JNIEnv,
    context: &JObject,
    method: &str,
    sig: &str,
    args: &[JValue],
) -> Option<String> {
    let file = match env.call_method(context, method, sig, args).and_then(|v| v.l()) {
        Ok(f) => f,
        Err(_) => {
            let _ = env.exception_clear();
            return None;
        }
    };
    let path = match env
        .call_method(&file, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
    {
        Ok(p) => p,
        Err(_) => {
            let _ = env.exception_clear();
            return None;
        }
    };
    match env.get_string(&JString::from(path)) {
        Ok(s) => Some(s.into()),
        Err(_) => {
            let _ = env.exception_clear();
            None
        }
    }
}

/// Append timestamped lines to both log files (best-effort).
fn write_lines(paths: &LogPaths, lines: &[String]) {
    let start = *START.get_or_init(Instant::now);
    for line in lines {
        let full = format!("+{:.3}s {line}\n", start.elapsed().as_secs_f64());
        let _ = append(&paths.internal, &full);
        if let Some(ext) = &paths.external {
            let _ = append(ext, &full);
        }
    }
}

fn append(path: &PathBuf, s: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(s.as_bytes())
}

/// Mirror the internal log to the public Downloads folder via
/// `io.reeda.app.LogExporter` (MediaStore on API 29+).
fn export() {
    let Some(paths) = PATHS.get() else {
        return;
    };
    let Ok(content) = std::fs::read_to_string(&paths.internal) else {
        return;
    };
    let ctx = ndk_context::android_context();
    if ctx.vm().is_null() || ctx.context().is_null() {
        return;
    }
    // SAFETY: `ctx.vm()`/`ctx.context()` are the runtime-owned JVM/Context
    // pointers set via ndk_context, non-null (checked above).
    let vm = match unsafe { JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) } {
        Ok(vm) => vm,
        Err(_) => return,
    };
    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(_) => return,
    };
    // SAFETY: `ctx.context()` is the app Context global reference owned by
    // the runtime; wrapped without freeing, valid for the call duration.
    let context = unsafe { JObject::from_raw(ctx.context() as jobject) };
    let Ok(name) = env.new_string(FILE_NAME) else {
        let _ = env.exception_clear();
        return;
    };
    let Ok(content) = env.new_string(&content) else {
        let _ = env.exception_clear();
        return;
    };
    let _ = env.call_static_method(
        "io/reeda/app/LogExporter",
        "export",
        "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&context), JValue::Object(&name), JValue::Object(&content)],
    );
    let _ = env.exception_clear();
    drop(env);
}