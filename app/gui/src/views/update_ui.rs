//! The update dialog and its download.

use super::*;

pub(super) fn mb(n: u64) -> String {
    format!("{:.1} MB", n as f64 / 1e6)
}

pub(super) fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

pub(super) fn set_update(ui: &AppWindow, state: &str, status: String, body: String, progress: f32) {
    let r = view(|v| v.update.clone());
    ui.global::<Api>().set_update(UpdateInfo {
        state: state.into(),
        version: r.as_ref().map_or(String::new(), |r| r.version.clone()).into(),
        status: format!("{VERSION}{}{status}", if status.is_empty() { "" } else { " · " }).into(),
        body: body.into(),
        progress,
        can_install: update::target().is_some() && r.is_some_and(|r| r.size().is_some()),
    });
}

/// The dialog's text for the newer release found.
pub(super) fn update_available(ui: &AppWindow) {
    let Some(r) = view(|v| v.update.clone()) else { return };
    let body = match (update::target(), r.size()) {
        (Some(t), Some(n)) => trf(
            "You have {}. The new version ({}) is downloaded from GitHub, checked against the release's SHA-256 checksums and replaces {}. Saves, snapshots and settings are not touched.",
            &[&VERSION, &mb(n), &file_name(&t)],
        ),
        (Some(_), None) => trf("You have {}. This release has no file for this system: see its page.", &[&VERSION]),
        (None, _) => trf(
            "You have {}. Only the AppImage and the Windows .exe replace themselves: download the new version from the release page.",
            &[&VERSION],
        ),
    };
    set_update(ui, "available", trf("{} is available", &[&r.version]), body, 0.0);
}

/// Ask GitHub for a newer version, off the UI thread. From Settings (`manual`) the result
/// shows there and a newer version opens the dialog; on start only a newer version is
/// told, in a toast.
pub fn check_update(ui: &AppWindow, manual: bool) {
    set_update(ui, "checking", tr("checking…").into(), String::new(), 0.0);
    let w = ui.as_weak();
    std::thread::spawn(move || {
        let res = update::check();
        let _ = w.upgrade_in_event_loop(move |ui| match res {
            Ok(Some(r)) => {
                let v = r.version.clone();
                view(|s| s.update = Some(r));
                update_available(&ui);
                if manual {
                    ui.global::<Api>().set_update_open(true);
                } else {
                    toast_full(&ui, trf("Version {} is available", &[&v]), "", tr("Update…"), ToastAct::Update, false);
                }
            }
            Ok(None) => set_update(&ui, "current", tr("up to date").into(), String::new(), 0.0),
            Err(e) => set_update(&ui, "error", e, String::new(), 0.0),
        });
    });
}

pub(super) fn wire_update(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    set_update(ui, "", String::new(), String::new(), 0.0);
    let w = ui.as_weak();
    api.on_check_update(move || {
        if let Some(ui) = w.upgrade() {
            check_update(&ui, true);
        }
    });
    api.on_update_page(|| {
        let r = view(|v| v.update.clone());
        system::open_url(r.as_ref().map_or("https://github.com/Scrimas/MHGU-Save-editor/releases", |r| &r.page));
    });
    let w = ui.as_weak();
    api.on_install_update(move || {
        let Some(ui) = w.upgrade() else { return };
        let (Some(r), Some(old)) = (view(|v| v.update.clone()), update::target()) else { return };
        let total = r.size().unwrap_or(0);
        set_update(&ui, "downloading", trf("downloading {}…", &[&r.version]), trf("Downloading… {} of {}", &[&mb(0), &mb(total)]), 0.0);
        let cancel = Arc::new(AtomicBool::new(false));
        view(|v| v.update_cancel = Some(cancel.clone()));
        let w = ui.as_weak();
        std::thread::spawn(move || {
            let last = std::cell::Cell::new(u64::MAX);
            let progress = |done, total| {
                let pct = done * 100 / u64::max(total, 1);
                if pct != last.replace(pct) && !cancel.load(Ordering::Relaxed) {
                    let (v, body) = (r.version.clone(), trf("Downloading… {} of {}", &[&mb(done), &mb(total)]));
                    let _ = w.upgrade_in_event_loop(move |ui| {
                        set_update(&ui, "downloading", trf("downloading {}…", &[&v]), body, pct as f32 / 100.0)
                    });
                }
            };
            let res = update::install(&r, &old, progress, &cancel);
            let cancelled = cancel.load(Ordering::Relaxed);
            let _ = w.upgrade_in_event_loop(move |ui| {
                view(|v| v.update_cancel = None);
                match res {
                    // settings live on this (the UI) thread
                    Ok(done) => {
                        if let Some(aside) = done.aside {
                            settings::update(|s| s.update_leftover = Some(aside));
                        }
                        #[cfg(target_os = "linux")]
                        crate::desktop::moved(&done.file, &r.version);
                        let body = trf("{} is in place. Restart to use it.", &[&file_name(&done.file)]);
                        view(|v| v.update_exe = Some(done.file));
                        set_update(&ui, "ready", trf("{} installed, restart to use it", &[&r.version]), body, 1.0);
                    }
                    Err(_) if cancelled => update_available(&ui),
                    Err(e) => set_update(&ui, "failed", tr("update failed").into(), trf("Not updated: {}. The editor you are running is unchanged.", &[&e]), 0.0),
                }
            });
        });
    });
    let w = ui.as_weak();
    api.on_cancel_update(move || {
        if let Some(c) = view(|v| v.update_cancel.take()) {
            c.store(true, Ordering::Relaxed);
        }
        if let Some(ui) = w.upgrade() {
            update_available(&ui);
            ui.global::<Api>().set_update_open(false);
            toast(&ui, tr("Update cancelled"), false);
        }
    });
    let w = ui.as_weak();
    let st = st.clone();
    api.on_restart_update(move || {
        let Some(ui) = w.upgrade() else { return };
        let Some(exe) = view(|v| v.update_exe.clone()) else { return };
        let save = st.borrow().doc.as_ref().map(|d| d.loc.opened.clone());
        match update::restart(&exe, save.as_deref()) {
            Ok(()) => ui.global::<Api>().invoke_quit(),
            Err(e) => toast(&ui, trf("Could not start {}: {}", &[&file_name(&exe), &e]), true),
        }
    });
}
