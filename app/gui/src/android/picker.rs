//! The system's pickers for the editor's file dialogs (platform.rs) and for the save
//! (source.rs). A request goes to MainActivity.pick and returns at once; the answer comes
//! back later through onPicked, on the UI thread, and runs what the request left here.
//! Blocking for it instead would hang the activity: Android pauses it while the picker
//! is shown, and that waits on this thread.

use crate::platform::Kind;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;

pub const FOLDER: i32 = 0;
pub const FILE: i32 = 1;
pub const FILES: i32 = 2;
pub const NEW_FILE: i32 = 3;

type Then = Box<dyn FnOnce(Result<Vec<String>, String>)>;
/// What open_file hands the file it read to: its name and bytes.
pub type Opened = Box<dyn FnOnce(Result<(PathBuf, Vec<u8>), String>)>;

thread_local! {
    static WAITING: RefCell<HashMap<i32, Then>> = RefCell::new(HashMap::new());
    static NEXT: Cell<i32> = const { Cell::new(1) };
}

/// Starts a picker of `kind`; `then` gets the URIs picked (not called when cancelled).
pub fn pick(kind: i32, mime: &str, name: &str, then: Then) {
    let id = NEXT.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    WAITING.with_borrow_mut(|w| w.insert(id, then));
    if let Err(e) = super::pick(id, kind, mime, name)
        && let Some(then) = WAITING.with_borrow_mut(|w| w.remove(&id))
    {
        then(Err(e));
    }
}

/// onPicked's answer to request `id` (mod.rs).
pub fn answered(id: i32, text: String) {
    let Some(then) = WAITING.with_borrow_mut(|w| w.remove(&id)) else { return };
    if let Some(why) = text.strip_prefix('!') {
        return then(Err(why.to_string()));
    }
    let uris: Vec<String> = text.lines().filter(|l| !l.is_empty()).map(str::to_string).collect();
    if !uris.is_empty() {
        then(Ok(uris));
    }
}

/// What a file picker filters on: anything for opening (a CSV or a save another app
/// typed oddly stays visible), the file's type for a new one.
fn mime_of(kind: Option<&Kind>) -> &'static str {
    match kind.and_then(|k| k.1.first()) {
        Some(&"csv") => "text/csv",
        Some(_) => "application/octet-stream",
        None => "*/*",
    }
}

/// platform::open_file on Android: the document picked, read through its URI.
pub fn open_file(_kinds: &[Kind], done: Opened) {
    pick(
        FILE,
        mime_of(None),
        "",
        Box::new(move |r| {
            done(r.and_then(|uris| {
                let u = &uris[0];
                let name = super::display_name(u);
                super::read(u).map(|b| (PathBuf::from(name), b)).map_err(|e| crate::i18n::trf("Could not read {}: {}", &[&super::display_name(u), &e]))
            }))
        }),
    );
}

/// platform::save_file on Android: a new document, then `bytes` written into it.
pub fn save_file(kind: Kind, name: &str, bytes: Vec<u8>, done: Box<dyn FnOnce(Result<(), String>)>) {
    pick(
        NEW_FILE,
        mime_of(Some(&kind)),
        name,
        Box::new(move |r| {
            done(r.and_then(|uris| {
                let u = &uris[0];
                super::write(u, &bytes).map_err(|e| crate::i18n::trf("Could not write {}: {}", &[&super::display_name(u), &e]))
            }))
        }),
    );
}
