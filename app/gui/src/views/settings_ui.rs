//! The Settings dialog.

use super::*;
use crate::theme;
use std::cell::RefCell;

/// The Settings dialog's values.
pub(super) fn settings_ui(ui: &AppWindow) {
    let s = settings::get();
    let pos = |v: &[u32], x: u32| v.iter().position(|&y| y == x).unwrap_or(0) as i32;
    let scale_now = match settings::scale_started() {
        None => "Set by SLINT_SCALE_FACTOR in the environment".to_string(),
        Some(n) if n != s.scale => "Applies the next time the editor starts".to_string(),
        Some(_) => String::new(),
    };
    let dir = system::snapshot_root();
    ui.global::<Api>().set_settings(SettingsInfo {
        theme: settings::THEMES.iter().position(|t| *t == s.theme).unwrap_or(0) as i32,
        scale: pos(&settings::SCALES, s.scale),
        scale_now: scale_now.into(),
        snapshot_dir: fmt::elide_path(&dir.display().to_string(), 56).into(),
        snapshot_custom: s.snapshot_dir.is_some(),
        keep: pos(&settings::KEEPS, s.keep_snapshots),
        snapshot_first: s.snapshot_first,
        folders: strings(s.save_folders.iter().map(|p| p.display().to_string())),
        emulators: strings(s.emulators.clone()),
        builtin: system::EMULATORS.join(", ").into(),
        reopen: s.reopen_last,
        recent: s.recent.len() as i32,
        confirmed_only: s.confirmed_only,
        check_updates: s.check_updates,
        matugen_offered: cfg!(target_os = "linux"),
        matugen: s.matugen,
        matugen_hint: MATUGEN.with(|m| m.borrow().hint.clone()).into(),
        matugen_template: theme::template_path().is_some_and(|p| p.exists()),
        matugen_snippet: theme::snippet().into(),
    });
}

/// What the matugen colours were last read from.
#[derive(Default)]
struct Matugen {
    /// matugen.json's modification time when last read; None to read it again
    seen: Option<std::time::SystemTime>,
    applied: bool,
    hint: String,
}

thread_local! {
    static MATUGEN: RefCell<Matugen> = RefCell::new(Matugen::default());
    static MATUGEN_WATCH: slint::Timer = slint::Timer::default();
}

/// Shows matugen's colours, or the built-in ones when off or unreadable. Reads matugen.json
/// only when it changed since the last look, or with `force` (the setting changed).
fn matugen(ui: &AppWindow, force: bool) {
    let on = cfg!(target_os = "linux") && settings::get().matugen;
    let path = theme::path();
    let mtime = path.as_ref().and_then(|p| std::fs::metadata(p).ok()?.modified().ok());
    if !force && MATUGEN.with(|m| m.borrow().seen == mtime && mtime.is_some()) {
        return;
    }
    let api = ui.global::<Api>();
    let (loaded, hint) = match (on, &path, mtime) {
        (false, ..) => (None, String::new()),
        (true, Some(p), Some(_)) => match theme::load(p) {
            Ok(t) => (Some(t), format!("Following {}, read again each time matugen writes it.", theme::tilde(p))),
            Err(e) => (None, format!("{} could not be read ({e}); the built-in colours are shown.", theme::tilde(p))),
        },
        _ => (None, "No colours from matugen yet. Save the template, add these lines to matugen's config.toml and run matugen (or change the wallpaper):".into()),
    };
    let ok = loaded.is_some();
    let was = MATUGEN.with(|m| m.borrow().applied);
    match loaded {
        Some(t) => {
            api.set_colors_dark(t.dark);
            api.set_colors_light(t.light);
            theme::set_accent(ui.window(), Some(t.accent));
        }
        None if was => theme::set_accent(ui.window(), None),
        None => {}
    }
    api.set_colors_on(ok);
    let hint_changed = MATUGEN.with(|m| {
        let mut m = m.borrow_mut();
        // an unreadable file (matugen halfway through writing it) is read again next time
        m.seen = if ok { mtime } else { None };
        m.applied = ok;
        let changed = m.hint != hint;
        m.hint = hint;
        changed
    });
    if hint_changed {
        settings_ui(ui);
    }
}

pub(super) fn wire_settings(ui: &AppWindow) {
    let api = ui.global::<Api>();
    matugen(ui, true);
    settings_ui(ui);
    let theme = settings::get().theme;
    if theme != "system" {
        api.set_force_scheme(theme.into());
    }
    if cfg!(target_os = "linux") {
        let w = ui.as_weak();
        MATUGEN_WATCH.with(|t| {
            t.start(slint::TimerMode::Repeated, std::time::Duration::from_secs(2), move || {
                if let Some(ui) = w.upgrade()
                    && settings::get().matugen
                {
                    matugen(&ui, false);
                }
            })
        });
    }
    let w = ui.as_weak();
    api.on_save_matugen_template(move || {
        let Some(ui) = w.upgrade() else { return };
        let Some(p) = theme::template_path() else { return };
        let r = std::fs::create_dir_all(p.parent().unwrap()).and_then(|_| std::fs::write(&p, theme::TEMPLATE));
        match r {
            Ok(()) => toast(&ui, format!("Template saved to {}", theme::tilde(&p)), false),
            Err(e) => toast(&ui, format!("Could not save {}: {e}", p.display()), true),
        }
        settings_ui(&ui);
    });
    let w = ui.as_weak();
    api.on_set_setting(move |key, v| {
        let Some(ui) = w.upgrade() else { return };
        let on = v != 0;
        let at = |list: &[u32]| list.get(v as usize).copied().unwrap_or(0);
        settings::update(|s| match key.as_str() {
            "theme" => s.theme = settings::THEMES.get(v as usize).unwrap_or(&"system").to_string(),
            "matugen" => s.matugen = on,
            "scale" => s.scale = at(&settings::SCALES),
            "keep" => s.keep_snapshots = at(&settings::KEEPS),
            "snapshot-default-dir" => s.snapshot_dir = None,
            "snapshot-first" => s.snapshot_first = on,
            "reopen" => s.reopen_last = on,
            "confirmed-only" => s.confirmed_only = on,
            "check-updates" => s.check_updates = on,
            _ => eprintln!("unknown setting {key:?}"),
        });
        if key == "theme" {
            ui.global::<Api>().set_force_scheme(settings::get().theme.into());
        }
        if key == "matugen" {
            matugen(&ui, true);
        }
        settings_ui(&ui);
    });
    let w = ui.as_weak();
    api.on_pick_snapshot_dir(move || {
        let Some(ui) = w.upgrade() else { return };
        if let Some(p) = rfd::FileDialog::new().set_title("Snapshot folder").set_directory(system::snapshot_root()).pick_folder() {
            settings::update(|s| s.snapshot_dir = Some(p));
            settings_ui(&ui);
            toast(&ui, "New snapshots go to the new folder; earlier ones stay where they are", false);
        }
    });
    let w = ui.as_weak();
    api.on_add_save_folder(move || {
        let Some(ui) = w.upgrade() else { return };
        let Some(p) = rfd::FileDialog::new().set_title("Folder with MHGU saves").pick_folder() else { return };
        settings::update(|s| {
            if !s.save_folders.contains(&p) {
                s.save_folders.push(p.clone());
            }
        });
        let before = ui.global::<Api>().get_detected().row_count();
        detected(&ui);
        settings_ui(&ui);
        let n = ui.global::<Api>().get_detected().row_count().saturating_sub(before);
        toast(&ui, if n == 0 { "No new save found in that folder".to_string() } else { format!("Found {}", count(n, "new save", "new saves")) }, false);
    });
    let w = ui.as_weak();
    api.on_remove_save_folder(move |i| {
        let Some(ui) = w.upgrade() else { return };
        settings::update(|s| {
            if (i as usize) < s.save_folders.len() {
                system::forget_folder(&s.save_folders.remove(i as usize));
            }
        });
        detected(&ui);
        settings_ui(&ui);
    });
    let w = ui.as_weak();
    api.on_add_emulator(move |name| {
        let Some(ui) = w.upgrade() else { return };
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        settings::update(|s| {
            if !s.emulators.iter().any(|e| e.eq_ignore_ascii_case(&name)) {
                s.emulators.push(name);
            }
        });
        settings_ui(&ui);
    });
    let w = ui.as_weak();
    api.on_remove_emulator(move |i| {
        let Some(ui) = w.upgrade() else { return };
        settings::update(|s| {
            if (i as usize) < s.emulators.len() {
                s.emulators.remove(i as usize);
            }
        });
        settings_ui(&ui);
    });
    let w = ui.as_weak();
    api.on_clear_recent(move || {
        let Some(ui) = w.upgrade() else { return };
        settings::update(|s| s.recent.clear());
        detected(&ui);
        settings_ui(&ui);
    });
}
