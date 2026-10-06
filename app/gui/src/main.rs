#![cfg_attr(windows, windows_subsystem = "windows")]

mod assets;
mod fmt;
mod goals;
#[cfg(target_os = "linux")]
mod scroll;
mod settings;
mod state;
mod system;
mod targets;
mod views;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use state::State;
use std::cell::RefCell;
use std::rc::Rc;

slint::include_modules!();

pub type Shared = Rc<RefCell<State>>;

pub fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

pub fn strings(v: impl IntoIterator<Item = String>) -> ModelRc<SharedString> {
    model(v.into_iter().map(SharedString::from).collect())
}

fn main() -> Result<(), slint::PlatformError> {
    // the interface size is read when the window is made
    settings::apply_scale();
    // Wayland app id / X11 class, so the desktop can match the window to its icon and
    // rules: it needs the platform to exist and no window yet.
    slint::BackendSelector::new().select()?;
    #[cfg(all(unix, not(target_os = "macos")))]
    slint::set_xdg_app_id("mhgu-save-editor")?;
    let ui = AppWindow::new()?;
    #[cfg(target_os = "linux")]
    scroll::install(ui.window());
    let st: Shared = Rc::new(RefCell::new(State::default()));
    let api = ui.global::<Api>();
    api.set_build_info(
        format!(
            "Version {} · EU/western build (0100770008DD8000) · {}",
            env!("CARGO_PKG_VERSION"),
            if assets::available() { "game assets included" } else { "built without game assets" }
        )
        .into(),
    );
    api.set_assets_ok(assets::available());
    views::wire(&ui, &st);
    // quitting with staged changes asks first (S11)
    {
        let w = ui.as_weak();
        let st = st.clone();
        ui.window().on_close_requested(move || {
            let Some(ui) = w.upgrade() else { return slint::CloseRequestResponse::HideWindow };
            if !st.borrow().ops.is_empty() && !views::quitting() {
                ui.global::<Api>().set_quit_open(true);
                return slint::CloseRequestResponse::KeepWindowShown;
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    // arguments: save file ("-" for none), start page, snapshot steps (used for screenshots)
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(p) = args.first().filter(|p| *p != "-") {
        views::open(&ui, &st, std::path::Path::new(p));
    } else if args.is_empty() && settings::get().reopen_last {
        if let Some(p) = settings::get().recent.first().filter(|p| p.is_file()) {
            views::open(&ui, &st, p);
        }
    }
    if let Some(page) = args.get(1) {
        api.set_page(page.as_str().into());
        views::refresh(&ui, &st.borrow());
        // the "Save opened" toast would cover the page; toasts of the steps stay
        api.set_toast("".into());
        steps(&ui, page, args.get(2).map_or("", String::as_str));
    }
    // MHGU_SNAPSHOT=out.png: render the window to a PNG and quit (for checking the UI
    // without capturing the screen). Waits until the compositor has sized the window:
    // maximized, or the same size for a second (a sized floating window, e.g. parked
    // off-screen), at most 8 s (a slow first page delays mapping); then it settles.
    let snapshot_timer = slint::Timer::default();
    if let Some(out) = std::env::var_os("MHGU_SNAPSHOT") {
        let w = ui.as_weak();
        let start = std::time::Instant::now();
        let ready = std::cell::Cell::new(None::<std::time::Instant>);
        let size = std::cell::Cell::new((slint::PhysicalSize::default(), 0u32));
        snapshot_timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(250), move || {
            let Some(ui) = w.upgrade() else { return };
            let t = start.elapsed().as_millis();
            let now = ui.window().size();
            let (last, same) = size.get();
            size.set((now, if now == last { same + 1 } else { 0 }));
            if ready.get().is_none() && t >= 1500 && (ui.window().is_maximized() || same >= 4 || t >= 8000) {
                ready.set(Some(std::time::Instant::now()));
            }
            if !ready.get().is_some_and(|r| r.elapsed().as_millis() >= 800) {
                return;
            }
            match ui.window().take_snapshot() {
                Ok(buf) => {
                    let img = image::RgbaImage::from_raw(buf.width(), buf.height(), buf.as_bytes().to_vec()).unwrap();
                    if let Err(e) = img.save_with_format(&out, image::ImageFormat::Png) {
                        eprintln!("snapshot: {e}");
                    }
                }
                Err(e) => eprintln!("snapshot: {e}"),
            }
            let _ = slint::quit_event_loop();
        });
    }
    ui.run()
}

/// Put the UI in a given state for a screenshot: comma-separated steps run in order.
///   slot:N  tab:N  sel:N  store:N  owner:N  filter:F  large  missing  search:S  add:<category>:<id>
///   goal:<id>  char:<field>:<value>  monster:<index>:<field>:<value>  item:<slot>:<id>:<count>
///   goto:<key>  undo-all  review  write  dowrite  toastact  snapshots  quit  popup:<name>
///   theme:light|dark
fn steps(ui: &AppWindow, page: &str, list: &str) {
    let api = ui.global::<Api>();
    for step in list.split(',').filter(|s| !s.is_empty()) {
        let num = |s: &str| s.parse::<i32>().unwrap_or(-1);
        let parts: Vec<&str> = step.split(':').collect();
        match (page, &parts[..]) {
            (_, &["slot", n]) => api.invoke_select_slot(num(n)),
            ("quests", &["tab", n]) => api.invoke_select_quest_tab(num(n)),
            (_, &["tab", n]) => api.invoke_select_collection(num(n)),
            ("palicoes", &["sel", n]) => api.invoke_select_palico(num(n)),
            ("advanced", &["sel", n]) => api.invoke_select_field(num(n)),
            (_, &["sel", n]) => api.invoke_select_equip(num(n)),
            (_, &["store", n]) => {
                api.set_item_store(num(n));
                api.invoke_filter_items("".into());
            }
            (_, &["owner", n]) => {
                api.set_equip_owner(num(n));
                api.invoke_filter_equip(api.get_equip_filter());
            }
            ("requests", &["filter", f]) => {
                api.set_request_filter(f.into());
                api.invoke_filter_requests();
            }
            ("advanced", &["filter", f]) => api.invoke_filter_fields(f.into()),
            (_, &["filter", f]) => {
                api.set_equip_filter(f.into());
                api.invoke_filter_equip(f.into());
            }
            (_, &["large"]) => {
                api.set_monster_large_only(true);
                api.invoke_filter_monsters();
            }
            ("quests", &["missing"]) => {
                api.set_quest_missing(true);
                api.invoke_filter_quests();
            }
            ("collections", &["missing"]) => {
                api.set_check_missing(true);
                api.invoke_filter_checks();
            }
            ("monsters", &["missing"]) => {
                api.set_monster_missing(true);
                api.invoke_filter_monsters();
            }
            ("quests", &["search", q]) => {
                api.set_quest_search(q.into());
                api.invoke_filter_quests();
            }
            ("equipment", &["search", q]) => {
                api.set_equip_search(q.into());
                api.invoke_search_equip_list(q.into());
            }
            (_, &["add", cat, id]) => {
                if let Some(c) = api.get_equip_categories().iter().position(|c| c == cat) {
                    api.set_equip_category(c as i32);
                    api.invoke_put_equip(-1, num(id));
                }
            }
            (_, &["goal", ..]) => api.invoke_apply_preview(parts[1..].join(":").into()),
            (_, &["char", field, v]) => api.invoke_set_character(field.into(), num(v)),
            (_, &["monster", i, field, v]) => api.invoke_set_monster(num(i), field.into(), num(v)),
            (_, &["item", slot, id, n]) => api.invoke_set_item(num(slot), num(id), num(n)),
            (_, &["goto", ..]) => api.invoke_goto(parts[1..].join(":").into()),
            (_, &["undo-all"]) => api.invoke_undo_all(),
            (_, &["review"]) => api.set_review_open(true),
            (_, &["write"]) => {
                api.invoke_check_emulator();
                api.set_write_open(true);
            }
            // the Write dialog's blocked state without starting the emulator
            (_, &["running"]) => {
                api.set_emulator_running(true);
                api.set_emulator_name("Ryujinx".into());
            }
            (_, &["snapshots"]) => {
                api.invoke_list_snapshots();
                api.set_snapshots_open(true);
            }
            (_, &["quit"]) => api.set_quit_open(true),
            // write with a snapshot (point XDG_DATA_HOME elsewhere for tests), then press
            // the toast's button
            (_, &["dowrite"]) => {
                api.invoke_write(true);
            }
            (_, &["toastact"]) => api.invoke_toast_act(),
            (_, &["theme", s]) => api.set_force_scheme(s.into()),
            // after the page exists and has its size
            (_, &["popup", name]) => {
                let (w, name) = (ui.as_weak(), SharedString::from(name));
                slint::Timer::single_shot(std::time::Duration::from_millis(400), move || {
                    if let Some(ui) = w.upgrade() {
                        ui.global::<Api>().set_snapshot_popup(name.clone());
                    }
                });
            }
            _ => eprintln!("unknown step {step:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Drive the real UI callbacks headless. Needs MHGU_TEST_SAVE (a copy of `0/system`);
    //! writes only to a temporary copy of its folder.
    use super::*;
    use slint::Model;

    #[test]
    fn edit_review_undo_write() {
        let Some(p) = std::env::var_os("MHGU_TEST_SAVE") else { return };
        i_slint_backend_testing::init_no_event_loop();
        let src = std::path::Path::new(&p).parent().unwrap().parent().unwrap();
        let dir = std::env::temp_dir().join(format!("mhgu-ui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for c in ["0", "1"] {
            std::fs::create_dir_all(dir.join(c)).unwrap();
            for f in ["system", "system_backup"] {
                std::fs::copy(src.join(c).join(f), dir.join(c).join(f)).unwrap();
            }
        }
        let ui = AppWindow::new().unwrap();
        let st: Shared = Rc::new(RefCell::new(State::default()));
        views::wire(&ui, &st);
        views::open(&ui, &st, &dir.join("0/system"));
        let api = ui.global::<Api>();
        assert!(api.get_loaded());
        assert_eq!(api.get_write_targets().row_count(), 4);

        api.set_page("monsters".into());
        views::refresh(&ui, &st.borrow());
        let row = |i: i32| api.get_monsters().iter().find(|m| m.index == i).unwrap();
        let old = row(48).hunts;
        // three spin-box steps on one field merge into one edit
        for v in [old + 1, old + 2, 777] {
            api.invoke_set_monster(48, "hunts".into(), v);
        }
        assert_eq!(api.get_change_count(), 1);
        assert_eq!(row(48).hunts, 777);
        assert!(row(48).changed);
        // the page shows the file's value; Review lists old → new; the nav counts it
        assert_eq!(row(48).was_hunts.as_str(), fmt::num(old));
        let c = api.get_changes().row_data(0).unwrap();
        assert!(c.single && c.old.as_str() == fmt::num(old) && c.new == "777" && c.title.ends_with("· Hunted"));
        assert_eq!(api.get_page_counts().row_data(8), Some(1));
        assert_eq!(api.get_value_count(), 1);
        api.invoke_set_monster(4, "captures".into(), 5);
        assert_eq!(api.get_change_count(), 2);
        // undo the first edit only: Arzuros back, Rathalos kept
        let first = api.get_changes().iter().last().unwrap().id;
        api.invoke_undo(first);
        assert_eq!(api.get_change_count(), 1);
        assert_eq!(row(48).hunts, old);
        assert_eq!(row(4).captures, 5);
        // an edit back to the original value disappears
        let orig4 = st.borrow().save().original().to_vec();
        api.invoke_set_monster(4, "captures".into(), {
            let s = mhgu_save::Save::from_bytes(orig4).unwrap();
            mhgu_save::monsters::get(&s, s.base(0), 4).captures as i32
        });
        assert_eq!(api.get_change_count(), 0);

        // equipment: add to the first free slot, replace it (one merged edit), remove it
        // again (no edit left); a worn slot is refused
        api.set_page("equipment".into());
        views::refresh(&ui, &st.borrow());
        let free = api.get_equip_free();
        assert!(free >= 0);
        let head = api.get_equip_categories().iter().position(|c| c == "Head").unwrap();
        api.set_equip_category(head as i32);
        api.invoke_search_equip("".into());
        let picks: Vec<i32> = api.get_pick_equip().iter().map(|p| p.id).collect();
        assert!(picks.len() > 100 && !picks.contains(&0));
        api.invoke_put_equip(-1, picks[0]);
        assert_eq!(api.get_change_count(), 1);
        assert_eq!((api.get_equip_detail().slot, api.get_equip_detail().kind.as_str()), (free, "Head"));
        api.invoke_put_equip(free, picks[1]);
        assert_eq!(api.get_change_count(), 1);
        assert_eq!(api.get_equip_detail().id, picks[1]);
        api.invoke_put_equip(free, 0);
        assert_eq!(api.get_change_count(), 0);
        let worn = {
            let s = st.borrow();
            s.save().u16(s.base() + mhgu_save::equipment::WORN) as i32
        };
        api.invoke_put_equip(worn, 0);
        assert_eq!(api.get_change_count(), 0);

        // a goal is one edit of several values; one of them goes back on its own
        let money = goals::plan("money", st.borrow().save(), st.borrow().slot);
        api.invoke_apply_preview("money".into());
        assert_eq!(api.get_change_count(), if money.lines.is_empty() { 0 } else { 1 });
        let wyc = |s: &State| mhgu_save::character::get(s.save(), s.base()).wycademy;
        let wyc0 = {
            let s = st.borrow();
            mhgu_save::character::get(s.orig(), s.base()).wycademy
        };
        if money.lines.len() > 1 && wyc0 != mhgu_save::character::MAX_POINTS {
            api.invoke_undo_value("wycademy".into());
            assert_eq!(api.get_change_count(), 1);
            assert_eq!(wyc(&st.borrow()), wyc0);
            assert_eq!(st.borrow().ops[0].values.len(), money.lines.len() - 1);
        }
        // Undo all, then Redo brings the edits back
        let n = api.get_change_count();
        api.invoke_undo_all();
        assert_eq!(api.get_change_count(), 0);
        assert_eq!(st.borrow_mut().redo_all(), n as usize);
        views::refresh(&ui, &st.borrow());
        assert_eq!(api.get_change_count(), n);
        if system::running_emulators().is_empty() {
            assert!(api.invoke_write(false));
            assert_eq!(api.get_change_count(), 0);
            let s = mhgu_save::Save::from_bytes(std::fs::read(dir.join("1/system_backup")).unwrap()).unwrap();
            assert_eq!(mhgu_save::character::get(&s, s.base(0)).funds, 9_999_999);
            // after the write the file's values are the new "was" values: nothing staged
            assert_eq!(api.get_value_count(), 0);
        }

        // Confirmed only refuses a Derived edit (requests); off again, it goes through
        api.set_page("requests".into());
        api.set_request_filter("all".into());
        api.invoke_filter_requests();
        let r = api.get_requests().iter().find(|r| r.has_flags).unwrap();
        let before = api.get_change_count();
        settings::update(|s| s.confirmed_only = true);
        api.invoke_set_request(r.index, "accepted".into(), !r.accepted);
        assert_eq!(api.get_change_count(), before);
        settings::update(|s| s.confirmed_only = false);
        api.invoke_set_request(r.index, "accepted".into(), !r.accepted);
        assert_eq!(api.get_change_count(), before + 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
