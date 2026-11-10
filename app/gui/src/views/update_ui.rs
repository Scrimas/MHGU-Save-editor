//! The update dialog and its download.

use super::*;

pub(super) fn mb(n: u64) -> String {
    format!("{:.1} MB", n as f64 / 1e6)
}

pub(super) fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// An older version than this one.
fn older(v: &str) -> bool {
    update::version(v) < update::version(VERSION)
}

/// Android's installer takes only a newer version over the app: going back means
/// uninstalling first, from the release page's APK.
const ANDROID: bool = cfg!(target_os = "android");

pub(super) fn set_update(ui: &AppWindow, state: &str, status: String, body: String, progress: f32) {
    let r = view(|v| v.update.clone());
    ui.global::<Api>().set_update(UpdateInfo {
        state: state.into(),
        version: r.as_ref().map_or(String::new(), |r| r.version.clone()).into(),
        status: format!("{VERSION}{}{status}", if status.is_empty() { "" } else { " · " }).into(),
        body: body.into(),
        progress,
        can_install: update::target().is_some() && r.as_ref().is_some_and(|r| r.size().is_some() && !(ANDROID && older(&r.version))),
        older: r.as_ref().is_some_and(|r| older(&r.version)),
        updates_itself: r.as_ref().is_none_or(|r| r.updates_itself()),
    });
}

/// The dialog's text for the release it is about: the newer one found, or one picked in
/// Settings, older or newer.
pub(super) fn update_available(ui: &AppWindow) {
    let Some(r) = view(|v| v.update.clone()) else { return };
    let back = older(&r.version);
    let mut body = match (update::target(), r.size()) {
        _ if ANDROID && back => trf(
            "You have {}. Android installs only newer versions over the app. To go back to {}, uninstall the app (its settings and snapshots go with it, your saves stay), then install that version's APK from its release page.",
            &[&VERSION, &r.version],
        ),
        (Some(_), Some(n)) if ANDROID => trf(
            "You have {}. The new version ({}) is downloaded from GitHub and checked against the release's SHA-256 checksums, then Android asks you to install it. Saves, snapshots and settings are not touched.",
            &[&VERSION, &mb(n)],
        ),
        (Some(t), Some(n)) if back => trf(
            "You have {}. Version {} ({}) is downloaded from GitHub, checked against the release's SHA-256 checksums and replaces {}. Saves and snapshots are not touched.",
            &[&VERSION, &r.version, &mb(n), &file_name(&t)],
        ),
        (Some(t), Some(n)) => trf(
            "You have {}. The new version ({}) is downloaded from GitHub, checked against the release's SHA-256 checksums and replaces {}. Saves, snapshots and settings are not touched.",
            &[&VERSION, &mb(n), &file_name(&t)],
        ),
        (Some(_), None) => trf("You have {}. This release has no file for this system: see its page.", &[&VERSION]),
        (None, _) => trf("You have {}. Only the AppImage and the Windows .exe replace themselves: download this version from the release page.", &[&VERSION]),
    };
    if back && !ANDROID {
        // the published versions after it, up to this one
        let at = update::version(&r.version);
        let later = view(|v| {
            let after = |x: &&update::Release| update::version(&x.version) > at && update::version(&x.version) <= update::version(VERSION);
            v.versions.as_ref().map_or(1, |l| l.iter().filter(after).count())
        });
        body.push_str("\n\n");
        body.push_str(&trn(
            "What came in the {n} later release is gone until you update again. Your settings are copied first and come back when you return to {} or a newer version.",
            "What came in the {n} later releases is gone until you update again. Your settings are copied first and come back when you return to {} or a newer version.",
            later.max(1) as i64,
            &[&VERSION],
        ));
        if !r.updates_itself() {
            body.push_str("\n\n");
            body.push_str(&trf("{} cannot update itself: to come back, download a newer version from the release page.", &[&r.version]));
        }
    }
    let status = if back { trf("going back to {}", &[&r.version]) } else { trf("{} is available", &[&r.version]) };
    set_update(ui, "available", status, body, 0.0);
}

/// Android: the release's APK downloaded to `apk` and checked, then handed to the system's
/// installer, which asks the user, closes the editor and puts the new version in place.
fn apk_install(ui: &AppWindow, r: update::Release, apk: PathBuf) {
    if !system::may_install_apk() {
        let body = tr("Android first asks to let this app install apps: allow it on the page that opened, come back, then press Download and install again.");
        return set_update(ui, "available", trf("{} is available", &[&r.version]), body.into(), 0.0);
    }
    let total = r.size().unwrap_or(0);
    set_update(ui, "downloading", trf("downloading {}…", &[&r.version]), trf("Downloading… {} of {}", &[&mb(0), &mb(total)]), 0.0);
    let cancel = Arc::new(AtomicBool::new(false));
    view(|v| v.update_cancel = Some(cancel.clone()));
    let w = ui.as_weak();
    std::thread::spawn(move || {
        let last = std::cell::Cell::new(u64::MAX);
        let progress = |done, total| {
            let pct = done * 100 / u64::max(total, 1);
            if pct != last.replace(pct) && !cancel.load(Ordering::Relaxed) {
                let (v, body) = (r.version.clone(), trf("Downloading… {} of {}", &[&mb(done), &mb(total)]));
                let _ = w.upgrade_in_event_loop(move |ui| set_update(&ui, "downloading", trf("downloading {}…", &[&v]), body, pct as f32 / 100.0));
            }
        };
        let res = update::download_checked(&r, &apk, progress, &cancel);
        let cancelled = cancel.load(Ordering::Relaxed);
        let _ = w.upgrade_in_event_loop(move |ui| {
            view(|v| v.update_cancel = None);
            let failed =
                |e: String| set_update(&ui, "failed", tr("update failed").into(), trf("Not updated: {}. The editor you are running is unchanged.", &[&e]), 0.0);
            match res.and_then(|()| system::install_apk(&apk)) {
                Ok(true) => set_update(
                    &ui,
                    "installing",
                    trf("installing {}…", &[&r.version]),
                    tr("Android asks you to confirm the install. The editor then closes: open it again to use the new version.").into(),
                    1.0,
                ),
                // (allowed a moment ago, then taken back)
                Ok(false) => {
                    let _ = std::fs::remove_file(&apk);
                    let body = tr(
                        "Android first asks to let this app install apps: allow it on the page that opened, come back, then press Download and install again.",
                    );
                    set_update(&ui, "available", trf("{} is available", &[&r.version]), body.into(), 0.0);
                }
                Err(_) if cancelled => update_available(&ui),
                Err(e) => failed(e),
            }
        });
    });
}

#[cfg(target_os = "android")]
thread_local! {
    /// The window, for the installer's answer (`installed`).
    static UPDATE_UI: std::cell::RefCell<Option<Weak<AppWindow>>> = const { std::cell::RefCell::new(None) };
}

/// Android: what the system's installer did with the APK (PackageInstaller's status: -1 it
/// asks the user, 0 installed, 3 cancelled, other failures with Android's message).
#[cfg(target_os = "android")]
pub fn installed(status: i32, message: String) {
    let Some(ui) = UPDATE_UI.with_borrow(|u| u.as_ref().and_then(|w| w.upgrade())) else { return };
    if status == -1 {
        return;
    }
    if let Some(apk) = update::target() {
        let _ = std::fs::remove_file(apk);
    }
    match status {
        0 => set_update(&ui, "ready", tr("installed").into(), tr("Open the editor again to use the new version.").into(), 1.0),
        // the user said no
        3 => update_available(&ui),
        // another signature (a test build installed), or the app is newer
        5 => set_update(
            &ui,
            "failed",
            tr("update failed").into(),
            tr("Android refused it: the editor installed is signed with another key (a build of your own?). Uninstall it, then install the APK from the release page. Your saves stay; the settings and snapshots go with the app.").into(),
            0.0,
        ),
        _ => {
            let why = if message.is_empty() { trf("Android's installer answered {}", &[&status]) } else { message };
            set_update(&ui, "failed", tr("update failed").into(), trf("Not updated: {}. The editor you are running is unchanged.", &[&why]), 0.0);
        }
    }
}

/// The update dialog closed without installing: back to the newer release found, if any.
fn update_closed(ui: &AppWindow) {
    let state = ui.global::<Api>().get_update().state;
    if state == "downloading" || state == "ready" || state == "installing" {
        return;
    }
    let (latest, (state, status)) = view(|v| {
        v.update = v.latest.clone();
        (v.latest.is_some(), v.checked.clone())
    });
    if latest {
        update_available(ui);
    } else {
        set_update(ui, &state, status, String::new(), 0.0);
    }
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
                view(|s| {
                    s.latest = Some(r.clone());
                    s.update = Some(r);
                });
                update_available(&ui);
                if manual {
                    ui.global::<Api>().set_update_open(true);
                } else {
                    toast_full(&ui, trf("Version {} is available", &[&v]), "", tr("Update…"), ToastAct::Update, false);
                }
            }
            Ok(None) => {
                view(|s| s.checked = ("current".into(), tr("up to date").into()));
                set_update(&ui, "current", tr("up to date").into(), String::new(), 0.0);
            }
            Err(e) => {
                view(|s| s.checked = ("error".into(), e.clone()));
                set_update(&ui, "error", e, String::new(), 0.0);
            }
        });
    });
}

pub(super) fn wire_update(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    set_update(ui, "", String::new(), String::new(), 0.0);
    #[cfg(target_os = "android")]
    UPDATE_UI.set(Some(ui.as_weak()));
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
        if ANDROID {
            return apk_install(&ui, r, old);
        }
        let back = older(&r.version);
        // an older version rewrites the settings without the ones it does not know
        if back && !settings::keep_for_rollback() {
            let body = tr("Not installed: the settings file could not be copied. The editor you are running is unchanged.");
            set_update(&ui, "failed", tr("update failed").into(), body.into(), 0.0);
            return;
        }
        let stop_checks = back && r.updates_itself() && ui.global::<Api>().get_rollback_stop_checks();
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
                    let _ = w.upgrade_in_event_loop(move |ui| set_update(&ui, "downloading", trf("downloading {}…", &[&v]), body, pct as f32 / 100.0));
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
                        if stop_checks {
                            settings::update(|s| s.check_updates = false);
                            settings_ui(&ui);
                        }
                        #[cfg(target_os = "linux")]
                        crate::desktop::moved(&done.file, &r.version);
                        let body = trf("{} is in place. Restart to use it.", &[&file_name(&done.file)]);
                        view(|v| v.update_exe = Some(done.file));
                        set_update(&ui, "ready", trf("{} installed, restart to use it", &[&r.version]), body, 1.0);
                    }
                    Err(_) if cancelled => update_available(&ui),
                    Err(e) => {
                        set_update(&ui, "failed", tr("update failed").into(), trf("Not updated: {}. The editor you are running is unchanged.", &[&e]), 0.0)
                    }
                }
            });
        });
    });
    let w = ui.as_weak();
    api.on_update_closed(move || {
        if let Some(ui) = w.upgrade() {
            update_closed(&ui);
        }
    });
    wire_versions(ui);
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

/// "12 Oct 2026" from GitHub's "2026-10-12".
fn day(d: &str) -> String {
    let n: Vec<u32> = d.split('-').filter_map(|x| x.parse().ok()).collect();
    match n[..] {
        [y, m, dd] => fmt::date(dd as u8, m as u8, y as u16),
        _ => "—".into(),
    }
}

/// Settings' version list from what GitHub listed.
fn version_rows(ui: &AppWindow) {
    let Some(list) = view(|v| v.versions.clone()) else { return };
    let target = update::target().is_some();
    let rows: Vec<VersionRow> = list
        .iter()
        .enumerate()
        .map(|(i, r)| VersionRow {
            version: r.version.clone().into(),
            date: day(&r.date).into(),
            size: r.size().map_or("—".into(), mb).into(),
            current: r.version == VERSION,
            latest: i == 0,
            older: older(&r.version),
            updates_itself: r.updates_itself(),
            installable: target && r.size().is_some() && r.version != VERSION && !(ANDROID && older(&r.version)),
        })
        .collect();
    ui.global::<Api>().set_versions(model(rows));
}

/// The changes shown for version `i` of the list: from this version to it.
fn show_version(ui: &AppWindow, i: usize) {
    let api = ui.global::<Api>();
    api.set_version_sel(i as i32);
    let Some(list) = view(|v| v.versions.clone()) else { return };
    let Some(r) = list.get(i) else { return };
    if r.version == VERSION {
        api.set_version_changes(model(vec![]));
        api.set_version_changes_status(String::new().into());
        return;
    }
    match view(|v| v.changes.get(&r.version).cloned()) {
        Some(Ok(groups)) => {
            api.set_version_changes(model(change_lines(&groups)));
            api.set_version_changes_status(String::new().into());
        }
        Some(Err(e)) => {
            api.set_version_changes(model(vec![]));
            api.set_version_changes_status(e.into());
        }
        None => {
            api.set_version_changes(model(vec![]));
            api.set_version_changes_status(tr("Asking GitHub…").into());
            let (to, published): (String, Vec<String>) = (r.version.clone(), list.iter().map(|x| x.version.clone()).collect());
            let w = ui.as_weak();
            std::thread::spawn(move || {
                let res = update::changes(VERSION, &to, &published);
                let _ = w.upgrade_in_event_loop(move |ui| {
                    view(|v| v.changes.insert(to.clone(), res.map_err(|_| tr("No list of changes for this version").to_string())));
                    // only when still the one picked
                    let sel = ui.global::<Api>().get_version_sel() as usize;
                    if view(|v| v.versions.as_ref().and_then(|l| l.get(sel)).is_some_and(|x| x.version == to)) {
                        show_version(&ui, sel);
                    }
                });
            });
        }
    }
}

fn change_lines(groups: &[(String, Vec<String>)]) -> Vec<ChangeLine> {
    let mut out = vec![];
    for (v, titles) in groups {
        out.push(ChangeLine { head: true, text: v.clone().into() });
        if titles.is_empty() {
            out.push(ChangeLine { head: false, text: tr("Fixes and upkeep").into() });
        }
        out.extend(titles.iter().map(|t| ChangeLine { head: false, text: t.clone().into() }));
    }
    out
}

/// Settings opened on (or switched to) a section: the version list is read from GitHub
/// the first time Updates and versions shows.
pub fn settings_section(ui: &AppWindow) {
    if ui.global::<Api>().get_settings_section() != VERSIONS_SECTION || view(|v| v.versions.is_some() || v.versions_loading) {
        return;
    }
    view(|v| v.versions_loading = true);
    let api = ui.global::<Api>();
    api.set_versions_loading(true);
    api.set_versions_status(tr("Asking GitHub…").into());
    let w = ui.as_weak();
    std::thread::spawn(move || {
        let res = update::list();
        let _ = w.upgrade_in_event_loop(move |ui| {
            view(|v| v.versions_loading = false);
            let api = ui.global::<Api>();
            api.set_versions_loading(false);
            match res {
                Ok(list) => {
                    // picked first: the version before this one, the likely way back
                    let here = list.iter().position(|r| r.version == VERSION);
                    let first = match here {
                        Some(i) if i + 1 < list.len() => i + 1,
                        Some(i) => i,
                        None => 0,
                    };
                    view(|v| v.versions = Some(list));
                    api.set_versions_status(String::new().into());
                    version_rows(&ui);
                    show_version(&ui, first);
                    if let Some(i) = PENDING_INSTALL.take() {
                        api.invoke_install_version(i);
                    }
                }
                Err(e) => api.set_versions_status(e.into()),
            }
        });
    });
}

thread_local! {
    /// A screenshot step's version to open the install dialog for, once listed.
    static PENDING_INSTALL: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
}

/// The screenshot step `install:<n>`: the install dialog for version `n` of the list.
pub fn install_when_listed(ui: &AppWindow, i: i32) {
    if view(|v| v.versions.is_some()) {
        ui.global::<Api>().invoke_install_version(i);
    } else {
        PENDING_INSTALL.set(Some(i));
    }
}

/// Settings' section that lists the versions (the rail's order in app.slint).
const VERSIONS_SECTION: i32 = 5;

fn wire_versions(ui: &AppWindow) {
    let api = ui.global::<Api>();
    let w = ui.as_weak();
    api.on_settings_section_shown(move || {
        if let Some(ui) = w.upgrade() {
            settings_section(&ui);
        }
    });
    let w = ui.as_weak();
    api.on_pick_version(move |i| {
        if let Some(ui) = w.upgrade() {
            show_version(&ui, i as usize);
        }
    });
    let w = ui.as_weak();
    api.on_install_version(move |i| {
        let Some(ui) = w.upgrade() else { return };
        let Some(r) = view(|v| v.versions.as_ref().and_then(|l| l.get(i as usize).cloned())) else { return };
        if r.version == VERSION {
            return;
        }
        view(|v| v.update = Some(r));
        update_available(&ui);
        let api = ui.global::<Api>();
        api.set_rollback_stop_checks(true);
        api.set_update_open(true);
    });
    api.on_version_page(|i| {
        if let Some(r) = view(|v| v.versions.as_ref().and_then(|l| l.get(i as usize).cloned())) {
            system::open_url(&r.page);
        }
    });
    api.on_news_page(|| system::open_url("https://github.com/Scrimas/MHGU-Save-editor/releases"));
}

/// On start: after an update (Settings' "Show what changed after an update"), what came
/// since the version that ran before, read from GitHub.
pub fn show_changes(ui: &AppWindow) {
    let last = settings::get().last_version;
    if last != VERSION {
        settings::update(|s| s.last_version = VERSION.into());
    }
    if !settings::get().show_changes || update::version(&last).is_none() || !older(&last) {
        return;
    }
    let w = ui.as_weak();
    std::thread::spawn(move || {
        let res = update::list().and_then(|l| update::changes(&last, VERSION, &l.into_iter().map(|r| r.version).collect::<Vec<_>>()));
        let _ = w.upgrade_in_event_loop(move |ui| {
            if let Ok(groups) = &res
                && !groups.is_empty()
            {
                let api = ui.global::<Api>();
                api.set_news(model(change_lines(groups)));
                api.set_news_title(trf("What changed since {}", &[&last]).into());
            }
        });
    });
}
