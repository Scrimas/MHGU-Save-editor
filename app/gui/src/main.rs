#![cfg_attr(windows, windows_subsystem = "windows")]

mod assets;
mod goals;
#[cfg(target_os = "linux")]
mod scroll;
mod state;
mod system;
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
    api.set_detected(strings(system::detect_saves().into_iter().map(|p| p.display().to_string())));
    views::wire(&ui, &st);

    if let Some(p) = std::env::args_os().nth(1) {
        views::open(&ui, &st, std::path::Path::new(&p));
        // second argument: start page (used for screenshots)
        if let Some(page) = std::env::args().nth(2) {
            api.set_page(page.as_str().into());
            // third argument: tab of the Collections or Quests page; on the Equipment page
            // comma-separated steps: a box slot to select, add:<category>:<id>, picker
            let arg = std::env::args().nth(3).unwrap_or_default();
            if page == "equipment" {
                views::refresh(&ui, &st.borrow());
                for step in arg.split(',').filter(|s| !s.is_empty()) {
                    match step.split(':').collect::<Vec<_>>()[..] {
                        ["picker"] => {
                            let w = ui.as_weak();
                            slint::Timer::single_shot(std::time::Duration::from_millis(300), move || {
                                if let Some(ui) = w.upgrade() {
                                    ui.global::<Api>().set_equip_picker_request(-1);
                                }
                            });
                        }
                        ["add", cat, id] => {
                            if let Some(c) = api.get_equip_categories().iter().position(|c| c == cat) {
                                api.set_equip_category(c as i32);
                                api.invoke_put_equip(-1, id.parse().unwrap_or(0));
                            }
                        }
                        [slot] => api.invoke_select_equip(slot.parse().unwrap_or(-1)),
                        _ => {}
                    }
                }
            } else if let Ok(tab) = arg.parse() {
                if page == "quests" { api.invoke_select_quest_tab(tab) } else { api.invoke_select_collection(tab) }
            }
        }
    }
    // MHGU_SNAPSHOT=out.png: render the window to a PNG and quit (for checking the UI
    // without capturing the screen)
    if let Some(out) = std::env::var_os("MHGU_SNAPSHOT") {
        let w = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(1500), move || {
            let Some(ui) = w.upgrade() else { return };
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

        // a goal, then write without snapshot
        api.invoke_apply_goal("money".into());
        assert_eq!(api.get_change_count(), 1);
        if system::running_emulators().is_empty() {
            assert!(api.invoke_write(false));
            assert_eq!(api.get_change_count(), 0);
            let s = mhgu_save::Save::from_bytes(std::fs::read(dir.join("1/system_backup")).unwrap()).unwrap();
            assert_eq!(mhgu_save::character::get(&s, s.base(0)).funds, 9_999_999);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
