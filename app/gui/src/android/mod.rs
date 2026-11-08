//! Android: the entry point the APK's activity runs, the app's folders, logging, and JNI
//! to the Java side of the app (app/packaging/android/java/.../MainActivity.java: the
//! system's pickers, reading and writing the files they hand out, the wallpaper colours,
//! opening links).
//!
//! Java methods never throw: a failure comes back as `null` (then `lastError()`) or as a
//! string starting with "!".

pub mod back;
pub mod colors;
pub mod picker;
pub mod source;

use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::refs::Global;
use jni::{Env, EnvUnowned, JValue, JavaVM, NativeMethod, jni_sig, jni_str};
use slint::android::AndroidApp;
use std::ffi::{CString, c_char, c_void};
use std::sync::OnceLock;

/// The activity (a global reference), for calling its methods from the UI thread.
static ACTIVITY: OnceLock<Global<JObject<'static>>> = OnceLock::new();

unsafe extern "C" {
    fn __android_log_write(prio: i32, tag: *const c_char, text: *const c_char) -> i32;
}

/// A line in Android's log (adb logcat -s mhgu-editor).
pub fn log(text: &str) {
    let (tag, text) = (c"mhgu-editor", CString::new(text.replace('\0', " ")).unwrap_or_default());
    // 4: ANDROID_LOG_INFO
    unsafe { __android_log_write(4, tag.as_ptr(), text.as_ptr()) };
}

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    std::panic::set_hook(Box::new(|p| log(&format!("panic: {p}"))));
    // Settings, snapshots and the working copy of the save live in the app's own folder:
    // `dirs` follows these on Android as on Linux.
    let files = app.internal_data_path().unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp"));
    // SAFETY: first thing in the process, before Slint or anything else starts a thread
    unsafe {
        std::env::set_var("HOME", &files);
        std::env::set_var("XDG_CONFIG_HOME", files.join("config"));
        std::env::set_var("XDG_DATA_HOME", files.join("data"));
        std::env::set_var("XDG_CACHE_HOME", files.join("cache"));
    }
    if let Err(e) = bind(&app) {
        log(&format!("JNI: {e}"));
    }
    if let Err(e) = slint::android::init(app) {
        return log(&format!("Slint: {e:?}"));
    }
    if let Err(e) = crate::run() {
        log(&format!("run: {e}"));
    }
}

/// Keeps the activity and registers the native method its pickers answer through.
fn bind(app: &AndroidApp) -> Result<(), jni::errors::Error> {
    // SAFETY: the pointer android-activity hands out is the process's JavaVM
    let vm = JavaVM::singleton().unwrap_or_else(|_| unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut _) });
    vm.attach_current_thread(|env| -> Result<(), jni::errors::Error> {
        // SAFETY: the activity's global reference, alive while the activity is
        let act = unsafe { JObject::from_raw(env, app.activity_as_ptr() as jni::sys::jobject) };
        let class = env.get_object_class(&act)?;
        // SAFETY: each function matches its Java declaration: `static native void onPicked(int,
        // String)`, `static native void onBack(int, float, boolean)`
        let methods = unsafe {
            [
                NativeMethod::from_raw_parts(jni_str!("onPicked"), jni_str!("(ILjava/lang/String;)V"), on_picked as *mut c_void),
                NativeMethod::from_raw_parts(jni_str!("onBack"), jni_str!("(IFZ)V"), on_back as *mut c_void),
            ]
        };
        unsafe { env.register_native_methods(&class, &methods)? };
        let _ = ACTIVITY.set(env.new_global_ref(&act)?);
        Ok(())
    })
}

/// MainActivity.onPicked(request, uris): the picker's answer, on Android's main thread; the
/// URIs one per line, "" when cancelled, "!why" when there is no picker.
extern "system" fn on_picked<'local>(mut env: EnvUnowned<'local>, _class: JClass<'local>, request: jni::sys::jint, uris: JString<'local>) {
    let text = env.with_env(|env| -> jni::errors::Result<String> { uris.try_to_string(env) }).resolve::<jni::errors::LogErrorAndDefault>();
    let _ = slint::invoke_from_event_loop(move || picker::answered(request, text));
}

/// MainActivity.onBack(phase, progress, fromRight): the predictive back gesture (back.rs).
extern "system" fn on_back<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
    phase: jni::sys::jint,
    progress: jni::sys::jfloat,
    from_right: jni::sys::jboolean,
) {
    let _ = slint::invoke_from_event_loop(move || back::gesture(phase, progress, from_right));
}

/// Tells MainActivity whether Back has something to do in the app (back.rs).
pub fn set_back(on: bool) -> Result<(), String> {
    with_activity(|env, act| {
        env.call_method(act, jni_str!("setBack"), jni_sig!("(Z)V"), &[JValue::Bool(on)])?;
        Ok(())
    })
}

/// Runs `f` with the activity on this thread's JNI environment.
fn with_activity<T>(f: impl FnOnce(&mut Env, &JObject) -> jni::errors::Result<T>) -> Result<T, String> {
    let act = ACTIVITY.get().ok_or("the activity is not bound")?;
    let vm = JavaVM::singleton().map_err(|e| e.to_string())?;
    vm.attach_current_thread(|env| {
        let r = f(env, act.as_ref());
        if env.exception_check() {
            env.exception_describe();
            env.exception_clear();
        }
        r
    })
    .map_err(|e: jni::errors::Error| e.to_string())
}

/// A String the method returned, None for null.
fn string_of(env: &mut Env, v: JObject) -> jni::errors::Result<Option<String>> {
    if v.is_null() {
        return Ok(None);
    }
    let s = env.cast_local::<JString>(v)?;
    Ok(Some(s.try_to_string(env)?))
}

fn last_error(env: &mut Env, act: &JObject) -> jni::errors::Result<String> {
    let v = env.call_method(act, jni_str!("lastError"), jni_sig!("()Ljava/lang/String;"), &[])?.l()?;
    Ok(string_of(env, v)?.unwrap_or_default())
}

/// Starts the system's picker for request `request` (picker.rs), of `kind`: 0 a folder,
/// 1 a file, 2 one or more files, 3 a new file named `name`; `mime` filters or names it.
pub fn pick(request: i32, kind: i32, mime: &str, name: &str) -> Result<(), String> {
    with_activity(|env, act| {
        let (mime, name) = (env.new_string(mime)?, env.new_string(name)?);
        env.call_method(
            act,
            jni_str!("pick"),
            jni_sig!("(IILjava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Int(request), JValue::Int(kind), JValue::Object(mime.as_ref()), JValue::Object(name.as_ref())],
        )?;
        Ok(())
    })
}

/// A String-to-String method of the activity; "!why" answers become errors.
fn call_string(method: &'static jni::strings::JNIStr, arg: &str) -> Result<String, String> {
    let r = with_activity(|env, act| {
        let a = env.new_string(arg)?;
        let v = env.call_method(act, method, jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"), &[JValue::Object(a.as_ref())])?.l()?;
        string_of(env, v)
    })?;
    match r {
        Some(s) if s.starts_with('!') => Err(s[1..].to_string()),
        Some(s) => Ok(s),
        None => Err(String::new()),
    }
}

/// The bytes of a document.
pub fn read(uri: &str) -> Result<Vec<u8>, String> {
    with_activity(|env, act| {
        let u = env.new_string(uri)?;
        let v = env.call_method(act, jni_str!("read"), jni_sig!("(Ljava/lang/String;)[B"), &[JValue::Object(u.as_ref())])?.l()?;
        if v.is_null() {
            return Ok(Err(last_error(env, act)?));
        }
        let a = env.cast_local::<JByteArray>(v)?;
        Ok(Ok(env.convert_byte_array(&a)?))
    })?
}

/// Replaces a document's bytes with `data`.
pub fn write(uri: &str, data: &[u8]) -> Result<(), String> {
    with_activity(|env, act| {
        let (u, b) = (env.new_string(uri)?, env.byte_array_from_slice(data)?);
        let v = env
            .call_method(
                act,
                jni_str!("write"),
                jni_sig!("(Ljava/lang/String;[B)Ljava/lang/String;"),
                &[JValue::Object(u.as_ref()), JValue::Object(b.as_ref())],
            )?
            .l()?;
        string_of(env, v)
    })?
    .map_or(Ok(()), Err)
}

/// The name the system shows for a document ("system", "MHGU").
pub fn display_name(uri: &str) -> String {
    call_string(jni_str!("displayName"), uri).unwrap_or_default()
}

/// The save in a picked folder: its place ("Download/MHGU/save_0") and the documents of
/// `system` and (or "") `system_backup`. Looks in the folder, its save_0, and save/save_0.
pub fn find_save(tree: &str) -> Result<(String, String, String), String> {
    let s = call_string(jni_str!("findSave"), tree)?;
    let mut it = s.split('\n').map(str::to_string);
    match (it.next(), it.next(), it.next()) {
        (Some(label), Some(sys), bak) => Ok((label, sys, bak.unwrap_or_default())),
        _ => Err(s),
    }
}

/// Whether the app still holds the permission for a folder or file picked earlier.
pub fn can_access(uri: &str) -> bool {
    call_string(jni_str!("canAccess"), uri).is_ok_and(|s| s == "1")
}

/// The wallpaper's Material colours (Android 12 and newer): "name=aarrggbb" per line.
pub fn system_colors() -> String {
    call_string(jni_str!("colors"), "").unwrap_or_default()
}

/// Opens a web page in the browser.
pub fn open_url(url: &str) {
    if let Err(e) = call_string(jni_str!("openUrl"), url) {
        log(&format!("openUrl: {e}"));
    }
}

/// After the shared handlers are wired: the wallpaper's colours, and the system's pickers
/// in place of the desktop's file dialogs.
pub fn started(ui: &crate::AppWindow, st: &crate::Shared) {
    colors::apply(ui);
    source::wire(ui, st);
    back::wire(ui);
}

/// Opens the save the app had open last (Settings: open the last save at start).
pub fn reopen(ui: &crate::AppWindow, st: &crate::Shared) {
    source::reopen(ui, st);
}
