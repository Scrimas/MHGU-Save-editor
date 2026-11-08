//! Android 14's predictive back. MainActivity forwards the gesture (Back34.java) and the
//! interface moves the sheet or shrinks the screen Back would close (ui/mobile/app.slint);
//! when the interface says Back has nothing to do in the app (Api.back-enabled), Android
//! keeps the gesture for its own back-to-home animation.

use crate::{Api, AppWindow};
use slint::ComponentHandle;
use std::cell::RefCell;

thread_local! {
    static UI: RefCell<Option<slint::Weak<AppWindow>>> = const { RefCell::new(None) };
}

pub fn wire(ui: &AppWindow) {
    UI.set(Some(ui.as_weak()));
    ui.global::<Api>().on_back_enabled(|on| {
        if let Err(e) = super::set_back(on) {
            super::log(&format!("setBack: {e}"));
        }
    });
}

/// MainActivity.onBack: 0 started, 1 progressed, 2 cancelled, 3 done (the Back key follows).
pub fn gesture(phase: i32, progress: f32, from_right: bool) {
    let Some(ui) = UI.with_borrow(|u| u.as_ref().and_then(|w| w.upgrade())) else { return };
    let api = ui.global::<Api>();
    if phase != 1 {
        let what = ["started", "", "cancelled", "done"][phase.clamp(0, 3) as usize];
        super::log(&format!("back gesture: {what}, on {:?}", ui.get_back_target()));
    }
    if phase <= 1 {
        api.set_back_from_right(from_right);
        api.set_back_progress(progress.clamp(0.0, 1.0));
        api.set_back_active(true);
    } else {
        api.set_back_active(false);
    }
}
