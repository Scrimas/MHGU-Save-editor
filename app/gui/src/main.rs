#![cfg_attr(windows, windows_subsystem = "windows")]

mod arrows;
mod assets;
#[cfg(target_os = "linux")]
mod desktop;
mod fmt;
mod focus;
mod goals;
mod i18n;
mod nav;
#[cfg(target_os = "linux")]
mod scroll;
mod settings;
mod state;
mod system;
mod targets;
mod theme;
mod update;
mod views;
mod warnings;

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use state::State;
use std::cell::RefCell;
use std::rc::Rc;

slint::include_modules!();

pub type Shared = Rc<RefCell<State>>;

pub fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

/// `v` written into the list model `cur` in place when the row count is the same. Every
/// callback refreshes the page: a new model would make its list drop and rebuild every
/// row, losing the keyboard focus and re-placing the scrolled view (a virtual ListView
/// re-estimates its rows). Only the rows that changed update.
pub fn keep<T: Clone + PartialEq + 'static>(cur: ModelRc<T>, v: Vec<T>) -> ModelRc<T> {
    if let Some(m) = cur.as_any().downcast_ref::<VecModel<T>>()
        && m.row_count() == v.len()
    {
        for (i, x) in v.into_iter().enumerate() {
            if m.row_data(i).as_ref() != Some(&x) {
                m.set_row_data(i, x);
            }
        }
        return cur;
    }
    model(v)
}

pub fn strings(v: impl IntoIterator<Item = String>) -> ModelRc<SharedString> {
    model(v.into_iter().map(SharedString::from).collect())
}

fn main() -> Result<(), slint::PlatformError> {
    // the interface size is read when the window is made
    settings::apply_scale();
    #[cfg(target_os = "linux")]
    desktop::install();
    update::cleanup();
    // Wayland app id / X11 class, so the desktop can match the window to its icon and
    // rules: it needs the platform to exist and no window yet.
    slint::BackendSelector::new().select()?;
    #[cfg(all(unix, not(target_os = "macos")))]
    slint::set_xdg_app_id("mhgu-save-editor")?;
    let ui = AppWindow::new()?;
    // the language set, else the system's (Slint picks the same one for the window)
    i18n::set(&ui, i18n::index(&settings::get().language));
    #[cfg(target_os = "linux")]
    scroll::install(ui.window());
    #[cfg(not(target_os = "linux"))]
    focus::install(ui.window());
    let st: Shared = Rc::new(RefCell::new(State::default()));
    let api = ui.global::<Api>();
    api.set_assets_ok(assets::available());
    views::wire(&ui, &st);
    nav::install(&ui);
    arrows::install(&ui);
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
    } else if args.is_empty()
        && settings::get().reopen_last
        && let Some(p) = settings::get().recent.first().filter(|p| p.is_file())
    {
        views::open(&ui, &st, p);
    }
    if let Some(page) = args.get(1) {
        api.set_page(page.as_str().into());
        views::refresh(&ui, &st.borrow());
        // the "Save opened" toast would cover the page; toasts of the steps stay
        api.set_toast("".into());
        steps(&ui, page, args.get(2).map_or("", String::as_str));
    } else if settings::get().check_updates && std::env::var_os("MHGU_SNAPSHOT").is_none() {
        views::check_update(&ui, false);
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
///   slot:N  tab:N  sel:N  store:N  loadout:N  owner:N  filter:F  large  missing  search:S  add:<category>:<id>
///   goal:<id>  char:<field>:<value>  monster:<index>:<field>:<value>  item:<slot>:<id>:<count>
///   set:N  arts:<where>:<field>:<slot>:<value>  pigment:<where>:<part>:<hex>
///   unlock:<map * 1024 + bit>:<0|1>  pet:<k>:<name|costume|adopted>:<value>  moofah:<k>:<v>
///   goto:<key>  undo-all  review  write  dowrite  toastact  snapshots  quit  popup:<name>
///   theme:light|dark  update (asks GitHub, as Settings' Check now)
fn steps(ui: &AppWindow, page: &str, list: &str) {
    let api = ui.global::<Api>();
    for step in list.split(',').filter(|s| !s.is_empty()) {
        let num = |s: &str| s.parse::<i32>().unwrap_or(-1);
        let parts: Vec<&str> = step.split(':').collect();
        match (page, &parts[..]) {
            (_, &["slot", n]) => api.invoke_select_slot(num(n)),
            (_, &["characters"]) => api.set_characters_open(true),
            (_, &["copy", a, b]) => api.invoke_slot_copy(num(a), num(b)),
            ("quests", &["tab", n]) => api.invoke_select_quest_tab(num(n)),
            (_, &["tab", n]) => api.invoke_select_collection(num(n)),
            ("palicoes", &["sel", n]) => api.invoke_select_palico(num(n)),
            ("advanced", &["sel", n]) => api.invoke_select_field(num(n)),
            ("database", &["sel", n]) => api.invoke_select_db(num(n)),
            ("unlocks", &["sel", n]) => api.invoke_select_unlock(num(n)),
            (_, &["unlock", key, on]) => api.invoke_set_unlock(num(key), on == "1"),
            (_, &["pet", k, field, v]) => api.invoke_set_pet(num(k), field.into(), v.into()),
            (_, &["moofah", k, v]) => api.invoke_set_moofah(num(k), num(v)),
            ("unlocks", &["missing"]) => {
                api.set_unlock_missing(true);
                api.invoke_filter_unlocks();
            }
            (_, &["db", kind, n]) => api.invoke_open_db(kind.into(), num(n)),
            ("database", &["edit", kind, n]) => api.invoke_show_in_editor(kind.into(), num(n)),
            (_, &["sel", n]) => api.invoke_select_equip(num(n)),
            (_, &["store", n]) => {
                api.set_item_store(num(n));
                api.invoke_filter_items("".into());
            }
            (_, &["loadout", n]) => api.invoke_select_loadout(num(n)),
            (_, &["owner", n]) => {
                api.set_equip_owner(num(n));
                api.invoke_filter_equip(api.get_equip_filter());
            }
            ("requests", &["filter", f]) => {
                api.set_request_filter(f.into());
                api.invoke_filter_requests();
            }
            ("advanced", &["filter", f]) => api.invoke_filter_fields(f.into()),
            ("monsters", &["filter", f]) => {
                api.set_monster_filter(f.into());
                api.invoke_filter_monsters();
            }
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
            ("database", &["search", q]) => {
                api.set_db_search(q.into());
                api.invoke_filter_db();
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
            // the field may hold colons ("char:use:0:3:12", "char:quests:6:12")
            (_, &["char", ref field @ .., v]) if !field.is_empty() => api.invoke_set_character(field.join(":").into(), num(v)),
            (_, &["monster", i, field, v]) => api.invoke_set_monster(num(i), field.into(), num(v)),
            (_, &["arena", q, set, t]) => api.invoke_set_arena(num(q), num(set), num(t)),
            ("palicoes", &["entry", kind, slot, id, on]) => api.invoke_set_palico_entry(kind.into(), num(slot), num(id), on == "1"),
            ("palicoes", &["look", key, v]) => api.invoke_set_palico_look(key.into(), num(v)),
            ("character", &["look", key, v]) => api.invoke_set_look(key.into(), num(v)),
            ("equipment", &["set", n]) => api.invoke_select_set(num(n)),
            (_, &["arts", at, field, slot, v]) => api.invoke_set_arts(num(at), field.into(), num(slot), num(v)),
            (_, &["pigment", at, part, hex]) => api.invoke_set_pigment(num(at), num(part), hex.into()),
            ("character", &["colour", key, hex]) => api.invoke_set_look_colour(key.into(), hex.into()),
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
            (_, &["update"]) => views::check_update(ui, true),
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
    //! Drive the real UI callbacks headless. Needs MHGU_TEST_SAVE (a copy of `0/system`)
    //! and `cargo test -- --ignored`; writes only to a temporary copy of its folder.
    use super::*;
    use slint::Model;

    /// The test save's folder copied to a temporary one (`tag` names it) and opened in a
    /// headless window.
    fn opened(tag: &str) -> (AppWindow, Shared, std::path::PathBuf) {
        let p = std::env::var_os("MHGU_TEST_SAVE").expect("set MHGU_TEST_SAVE to a copy of 0/system");
        i_slint_backend_testing::init_no_event_loop();
        let src = std::path::Path::new(&p).parent().unwrap().parent().unwrap();
        let dir = std::env::temp_dir().join(format!("mhgu-ui-{tag}-{}", std::process::id()));
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
        (ui, st, dir)
    }

    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn characters_and_palicoes_move() {
        let (ui, st, dir) = opened("chars");
        let api = ui.global::<Api>();
        let used = |k: usize| st.borrow().save().slot_used(k);
        let pals = |k: usize| {
            let s = st.borrow();
            (0..mhgu_save::palico::LIST_N).filter(|&i| !mhgu_save::palico::is_empty(s.save(), s.save().base(k), i)).count()
        };
        let name = api.get_slots().row_data(0).unwrap().name;
        // slot 1 copied over slot 3: one edit, listed as the slot, old → new
        api.invoke_slot_copy(0, 2);
        assert_eq!(api.get_change_count(), 1);
        assert!(used(2));
        assert_eq!(api.get_slots().row_data(2).unwrap().name, name);
        let c = api.get_changes().row_data(0).unwrap();
        assert!(c.single && c.title == "Character slot 3" && c.new.starts_with(name.as_str()), "{c:?}");
        // a Palico of slot 1 into the copy, its equipment with it
        api.set_page("palicoes".into());
        views::refresh(&ui, &st.borrow());
        let n = pals(2);
        api.invoke_palico_copy(2);
        assert_eq!(api.get_change_count(), 2);
        assert_eq!(pals(2), n + 1);
        // Undo all: the file as read
        api.invoke_undo_all();
        assert!(!st.borrow().save().is_dirty());
        // a swap keeps the character shown: it moves to the other slot
        api.invoke_slot_swap(0, 2);
        assert_eq!(st.borrow().slot, 2);
        assert_eq!(api.get_slots().row_data(2).unwrap().name, name);
        api.invoke_undo_all();
        // delete: the slot is empty; Undo on the field puts it back
        api.invoke_select_slot(0);
        api.invoke_slot_delete(0);
        assert!(!used(0));
        api.invoke_undo_value("slot:0".into());
        assert!(used(0) && !st.borrow().save().is_dirty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// My Sets, Palico sets and what the hunter has on: each edit stages one value, the
    /// models follow, and Undo on the field puts the file's bytes back.
    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn sets_and_arts() {
        use mhgu_save::sets;
        let (ui, st, dir) = opened("sets");
        let api = ui.global::<Api>();
        let base = st.borrow().base();
        // an unused My Set takes what the hunter has on
        api.set_page("equipment".into());
        api.set_equip_owner(2);
        let free = (0..sets::MY_SETS_N).find(|&k| !sets::my_set(st.borrow().save(), base, k).used()).unwrap();
        api.invoke_select_set(free as i32);
        assert!(!api.get_set_detail().used);
        api.invoke_set_from_current();
        assert!(api.get_set_detail().used);
        assert_eq!(sets::my_set(st.borrow().save(), base, free).arts, sets::arts(st.borrow().save(), base));
        // its style, then its name; then no weapon: no class to check the arts against
        api.invoke_set_arts(0, "style".into(), 0, 1);
        assert_eq!(api.get_set_detail().arts.slots, 3);
        api.invoke_set_set_name("Test".into());
        assert_eq!(api.get_set_detail().name, "Test");
        api.invoke_set_set_piece(0, 0);
        assert_eq!(sets::my_set(st.borrow().save(), base, free).gear[0], sets::NO_BOX);
        api.invoke_undo_all();
        assert!(!st.borrow().save().is_dirty());
        // a Palico set's piece names it; Undo on the field
        api.set_equip_owner(3);
        api.invoke_select_set(0);
        if api.get_set_detail().pieces.row_data(0).unwrap().choices.row_count() > 1 {
            api.invoke_set_set_piece(0, 1);
            assert_eq!(api.get_set_detail().name, "Set 01");
            api.invoke_undo_value("palset:0".into());
            assert!(!st.borrow().save().is_dirty());
        }
        // what the hunter has on: a custom colour, a style with fewer slots
        api.set_page("character".into());
        views::refresh(&ui, &st.borrow());
        api.invoke_set_pigment(-1, 0, "#2060C0".into());
        assert_eq!(api.get_char_pigment().row_data(0).unwrap().hex, "#2060C0");
        assert!(!api.get_was_pigment().is_empty());
        api.invoke_set_arts(-1, "style".into(), 0, 3);
        assert_eq!(api.get_char_arts().slots, 1);
        api.invoke_undo_value("pigment".into());
        api.invoke_undo_value("arts".into());
        assert!(!st.borrow().save().is_dirty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The Unlocks page: an entry stages its map, a Lab upgrade takes its supply set
    /// along, the pets and Moofahs; each Undo puts the file's bytes back, and every
    /// unlock goal runs to nothing left.
    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn unlocks_page() {
        use mhgu_save::unlocks;
        let (ui, st, dir) = opened("unlocks");
        let api = ui.global::<Api>();
        let base = st.borrow().base();
        api.set_page("unlocks".into());
        views::refresh(&ui, &st.borrow());
        assert!(api.get_unlock_rows().row_count() > 15);
        // Lab upgrade 10 (Heal Supplies: Pro Kit) off: its supply set 9 follows
        let lab = unlocks::find("lab").unwrap();
        let installed = unlocks::on(st.borrow().save(), base, lab, 10);
        api.invoke_set_unlock((lab * 1024 + 10) as i32, !installed);
        assert_eq!(unlocks::on(st.borrow().save(), base, lab, 10), !installed);
        if installed {
            assert!(!st.borrow().save().bit(base + unlocks::SUPPLY, 9));
        }
        // a pet adopted, a Moofah petted to 6
        api.invoke_select_unlock(4);
        assert!(api.get_unlock_pets() && api.get_pets().row_count() == 4);
        let adopted = unlocks::adopted(st.borrow().save(), base, 1);
        api.invoke_set_pet(1, "adopted".into(), if adopted { "0" } else { "1" }.into());
        api.invoke_set_pet(2, "name".into(), "Bacon".into());
        assert_eq!(api.get_pets().row_data(2).unwrap().name, "Bacon");
        api.invoke_set_moofah(0, 6);
        assert_eq!(api.get_moofahs().row_data(0), Some(6));
        for key in ["unlock:lab", "pet:1", "pet:2", "moofahs"] {
            api.invoke_undo_value(key.into());
        }
        assert!(!st.borrow().save().is_dirty(), "{:?}", &st.borrow().save().diff()[..st.borrow().save().diff().len().min(6)]);
        // the unlock goals and a page-wide Unlock all
        let (mut s, slot) = (st.borrow().save().clone(), st.borrow().slot);
        for g in ["lab", "costumes", "songs", "trader", "coins", "combos", "gallery", "unlocks:all:14"] {
            goals::apply(g, &mut s, slot);
            assert!(goals::plan(g, &s, slot).targets.is_empty(), "{g} twice");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn edit_review_undo_write() {
        let (ui, st, dir) = opened("edit");
        let api = ui.global::<Api>();
        assert!(api.get_loaded());
        assert_eq!(api.get_write_targets().row_count(), 4);
        // the unlock goals find work on a played save, and after them nothing is left
        if assets::available() {
            let (mut s, slot) = (st.borrow().save().clone(), st.borrow().slot);
            for g in ["obtained", "smithy", "card"] {
                assert!(!goals::plan(g, &s, slot).targets.is_empty(), "{g}");
                goals::apply(g, &mut s, slot);
                assert!(goals::plan(g, &s, slot).targets.is_empty(), "{g} twice");
            }
        }

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
        assert_eq!(api.get_page_counts().row_data(targets::page_index("monsters")), Some(1));
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
        // a decoration that does not fit the free slots goes in with a warning, and out again
        if !assets::names().decos.is_empty() {
            let (s, base) = (st.borrow().save().clone(), st.borrow().base());
            let free_of = |i: usize| {
                let e = mhgu_save::equipment::get(&s, base, mhgu_save::equipment::Owner::Hunter, i);
                views::deco_slots(mhgu_save::equipment::Owner::Hunter, &e).unwrap_or(0).saturating_sub(views::deco_used(&e))
            };
            if let Some(i) = (0..mhgu_save::equipment::BOX_N).find(|&i| free_of(i) == 1) {
                api.invoke_select_equip(i as i32);
                api.invoke_set_equip("deco-add".into(), 2647); // Earplug Jwl 3: 3 slots
                assert_eq!(api.get_change_count(), 1);
                assert!(!api.get_equip_detail().warning.is_empty());
                assert!(!api.get_changes().row_data(0).unwrap().warning.is_empty(), "Review shows it too");
                api.invoke_set_equip("deco-remove".into(), api.get_equip_detail().decos.row_data(0).unwrap().index);
                assert_eq!(api.get_change_count(), 0);
                api.invoke_set_equip("deco-add".into(), 2638); // Antidote Jwl 1
                assert_eq!(api.get_change_count(), 1);
                assert_eq!(api.get_equip_detail().deco_used, api.get_equip_detail().deco_slots);
                api.invoke_set_equip("deco-remove".into(), api.get_equip_detail().decos.row_data(0).unwrap().index);
                assert_eq!(api.get_change_count(), 0);
            }
            // the Palico box takes gear like the hunter's
            api.set_equip_owner(1);
            views::refresh(&ui, &st.borrow());
            api.set_equip_category(0);
            api.invoke_search_equip("".into());
            let id = api.get_pick_equip().row_data(0).unwrap().id;
            api.invoke_put_equip(-1, id);
            assert_eq!(api.get_change_count(), 1);
            api.invoke_put_equip(api.get_equip_detail().slot, 0);
            assert_eq!(api.get_change_count(), 0);
            api.set_equip_owner(0);
        }

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

        // Confirmed only refuses a Derived edit, not a Confirmed one; off again, a Derived
        // one goes through
        settings::update(|s| s.confirmed_only = true);
        assert!(views::refused(&ui, state::Conf::Derived));
        assert!(!views::refused(&ui, state::Conf::Confirmed));
        settings::update(|s| s.confirmed_only = false);
        assert!(!views::refused(&ui, state::Conf::Derived));
        // requests are Confirmed: one goes through with Confirmed only on
        api.set_page("requests".into());
        api.set_request_filter("all".into());
        api.invoke_filter_requests();
        let r = api.get_requests().iter().find(|r| r.has_flags).unwrap();
        let before = api.get_change_count();
        settings::update(|s| s.confirmed_only = true);
        api.invoke_set_request(r.index, "accepted".into(), !r.accepted);
        settings::update(|s| s.confirmed_only = false);
        assert_eq!(api.get_change_count(), before + 1);

        // a Review entry of another character opens that character
        if st.borrow().save().slot_used(1) {
            api.invoke_select_slot(1);
            api.invoke_set_monster(48, "hunts".into(), 4321);
            api.invoke_select_slot(0);
            let key = api.get_changes().iter().map(|c| c.key).find(|k| k.starts_with("1|")).unwrap();
            api.invoke_goto(key);
            assert_eq!(st.borrow().slot, 1);
        }
        // Ctrl+Z takes back the latest change
        {
            use slint::platform::{Key, WindowEvent};
            let n = api.get_change_count();
            ui.show().unwrap();
            let w = ui.window();
            w.dispatch_event(WindowEvent::KeyPressed { text: Key::Control.into() });
            w.dispatch_event(WindowEvent::KeyPressed { text: "z".into() });
            w.dispatch_event(WindowEvent::KeyReleased { text: "z".into() });
            w.dispatch_event(WindowEvent::KeyReleased { text: Key::Control.into() });
            assert_eq!(api.get_change_count(), n - 1);
        }
        // opening a save with staged changes asks first
        let n = api.get_change_count();
        views::open(&ui, &st, &dir.join("0/system"));
        assert_eq!(api.get_open_ask().as_str(), dir.join("0/system").display().to_string());
        assert_eq!(api.get_change_count(), n);
        api.invoke_open_confirmed();
        assert_eq!(api.get_change_count(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The sidebar as a list: a click gives it the keyboard, plain arrows open the pages.
    #[test]
    fn sidebar_arrows() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = AppWindow::new().unwrap();
        let api = ui.global::<Api>();
        api.set_loaded(true);
        api.set_slots(model(vec![SlotInfo { used: true, ..Default::default() }; 3]));
        sidebar_keys(&ui);
    }

    /// The same with a save open: every page switch rebuilds the page's models.
    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn sidebar_arrows_with_save() {
        use slint::platform::Key;
        let (ui, _st, dir) = opened("sidebar");
        nav::install(&ui);
        arrows::install(&ui);
        ui.window().set_size(slint::LogicalSize::new(1400.0, 900.0));
        ui.show().unwrap();
        let api = ui.global::<Api>();
        // Ctrl+arrows: the pages and the tabs
        api.set_page("character".into());
        press(&ui, Key::DownArrow, true);
        assert_eq!(api.get_page(), "items");
        assert_eq!(api.get_item_store(), 0);
        press(&ui, Key::RightArrow, true);
        assert_eq!(api.get_item_store(), 1);
        press(&ui, Key::LeftArrow, true);
        press(&ui, Key::LeftArrow, true);
        assert_eq!(api.get_item_store(), 0);
        sidebar_keys(&ui);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Plain arrows walk a page's list: the row that gets the keyboard is picked, so its
    /// detail follows; Up comes back.
    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn arrows_walk_lists() {
        use slint::platform::Key;
        let (ui, _st, dir) = opened("arrows");
        arrows::install(&ui);
        ui.window().set_size(slint::LogicalSize::new(1400.0, 900.0));
        ui.show().unwrap();
        let api = ui.global::<Api>();
        let key = |k: Key| press(&ui, k, false);
        let lists: [(&str, &dyn Fn() -> i32); 2] = [("advanced", &|| api.get_field_sel()), ("palicoes", &|| api.get_palico().index)];
        for (page, sel) in lists {
            api.set_page(page.into());
            slint::platform::update_timers_and_animations();
            let before = sel();
            // from the window: through the fields above the list to its rows
            let mut n = 0;
            while sel() == before && n < 12 {
                key(Key::DownArrow);
                n += 1;
            }
            let first = sel();
            assert_ne!(first, before, "{page}: the arrows reach the list");
            key(Key::DownArrow);
            let second = sel();
            assert_ne!(second, first, "{page}: Down picks the next row");
            key(Key::UpArrow);
            assert_eq!(sel(), first, "{page}: Up comes back");
            // nothing focused for the next page
            let inner = i_slint_core::window::WindowInner::from_pub(ui.window());
            let focused = inner.focus_item.borrow().upgrade();
            if let Some(f) = focused {
                inner.set_focus_item(&f, false, i_slint_core::input::FocusReason::Programmatic);
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Walking a long list: the view glides ahead of the picked row, keeping at least a
    /// whole row in sight beyond it, and never jumps back the other way.
    #[test]
    #[ignore = "needs MHGU_TEST_SAVE"]
    fn arrows_scroll_margin() {
        use slint::platform::Key;
        let (ui, _st, dir) = opened("margin");
        arrows::install(&ui);
        ui.window().set_size(slint::LogicalSize::new(1400.0, 900.0));
        ui.show().unwrap();
        let api = ui.global::<Api>();
        let settle = || {
            for _ in 0..12 {
                i_slint_backend_testing::testing_backend::mock_elapsed_time(20);
            }
        };
        for page in ["advanced", "database"] {
            api.set_page(page.into());
            slint::platform::update_timers_and_animations();
            let mut last = 0.0;
            let mut moved = false;
            for (k, n) in [(Key::DownArrow, 40), (Key::UpArrow, 25)] {
                let down = k == Key::DownArrow;
                for i in 0..n {
                    press(&ui, k, false);
                    // half way through the glide, then settled
                    i_slint_backend_testing::testing_backend::mock_elapsed_time(60);
                    let Some((mid, _, _)) = arrows::probe(&ui) else { continue };
                    settle();
                    let (y, row, list) = arrows::probe(&ui).unwrap();
                    let line = format!("{page} {:?} {i}: view {last:.0} → {mid:.0} → {y:.0}, row {:.0}..{:.0} in {:.0}..{:.0}", k, row.min_y(), row.max_y(), list.min_y(), list.max_y());
                    if down {
                        assert!(last <= mid + 0.5 && mid <= y + 0.5, "{line}: moved back");
                        if y > 0.5 {
                            assert!(row.max_y() + row.height() <= list.max_y() + 0.5, "{line}: no row in sight below");
                        }
                    } else {
                        assert!(last + 0.5 >= mid && mid + 0.5 >= y, "{line}: moved back");
                        if y > 0.5 {
                            assert!(row.min_y() - row.height() >= list.min_y() - 0.5, "{line}: no row in sight above");
                        }
                    }
                    moved |= y > 0.5;
                    last = y;
                }
            }
            assert!(moved, "{page}: the list scrolled");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A key press as winit hands it over: the focus hook first, Ctrl around it if asked.
    fn press(ui: &AppWindow, k: slint::platform::Key, ctrl: bool) {
        use slint::platform::{Key, WindowEvent};
        let send = |k: Key, down: bool| {
            let text = SharedString::from(k);
            ui.window().dispatch_event(if down { WindowEvent::KeyPressed { text } } else { WindowEvent::KeyReleased { text } });
        };
        arrows::refocus(ui.window());
        if ctrl {
            send(Key::Control, true);
        }
        send(k, true);
        send(k, false);
        if ctrl {
            send(Key::Control, false);
        }
        slint::platform::update_timers_and_animations();
    }

    /// Clicks a page of the sidebar as winit does (the focus hook first), then walks the
    /// pages with plain arrows.
    fn sidebar_keys(ui: &AppWindow) {
        use slint::platform::{Key, PointerEventButton, WindowEvent};
        ui.window().set_size(slint::LogicalSize::new(1400.0, 900.0));
        ui.show().unwrap();
        let api = ui.global::<Api>();
        let tick = || slint::platform::update_timers_and_animations();
        let key = |k: Key| press(ui, k, false);
        let at = |i: &str| targets::PAGES.iter().position(|p| p.0 == i);
        tick();
        // a click in the middle of the sidebar's pages (a 2 px gap between two misses)
        for y in [240.0, 244.0] {
            let position = slint::LogicalPosition::new(60.0, y);
            ui.window().dispatch_event(WindowEvent::PointerMoved { position });
            focus::press(ui.window(), i_slint_core::lengths::LogicalPoint::new(60.0, y));
            ui.window().dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
            ui.window().dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
            tick();
        }
        let i = at(&api.get_page()).expect("the click opens a page of the sidebar");
        assert!((1..targets::PAGES.len() - 3).contains(&i), "clicked page {i}");
        key(Key::DownArrow);
        assert_eq!(at(&api.get_page()), Some(i + 1));
        key(Key::DownArrow);
        assert_eq!(at(&api.get_page()), Some(i + 2));
        key(Key::UpArrow);
        key(Key::UpArrow);
        key(Key::UpArrow);
        assert_eq!(at(&api.get_page()), Some(i - 1));
        key(Key::End);
        assert_eq!(api.get_page(), "advanced");
        key(Key::DownArrow);
        assert_eq!(api.get_page(), "advanced");
        key(Key::Home);
        assert_eq!(api.get_page(), "overview");
    }
}
