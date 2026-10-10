//! Models for each page and the callbacks that edit the save.

use crate::assets;
use crate::fmt::{self, num};
use crate::goals;
use crate::i18n::{tr, trf, trn};
use crate::settings;
use crate::state::{Conf, Edit, State};
use crate::system;
use crate::targets::{self, Mon, Target, PAGES};
use crate::update::{self, VERSION};
use crate::{keep, model, strings, Shared};
use crate::{
    Api, AppWindow, AppearanceInfo, ArenaRow, ArtRow, ArtsInfo, CardInfo, ColourRow, ChangeRow, CharacterInfo, CheckRow, Confidence, DecoRow, DetectedSave, DeviantRow,
    EquipDetail, EquipRow, FieldRow, Goal, ItemSlot, LoadoutRow, MonsterRow, LookRow, PalicoDetail, PalicoEntry, PalicoRow, PickItem, PigmentRow, Preview,
    PreviewLine, QuestRow, RequestRow, SetDetail, SetPiece, SetRow, SettingsInfo, SlotInfo, SnapRow, StatCard, UpdateInfo, ValueLine, WeaponUseRow, WriteRow,
};
use mhgu_save::data::tables;
use mhgu_save::equipment::{self, Kind, Owner};
use mhgu_save::items::{self, Stack, Store};
use mhgu_save::progress::{Char, Lock, QuestBit, DEVIANTS};
use mhgu_save::{character, monsters, palico, store};
use slint::{ComponentHandle, Image, Model, SharedString, Weak};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn conf(c: Conf) -> Confidence {
    match c {
        Conf::Confirmed => Confidence::Confirmed,
        Conf::Derived => Confidence::Derived,
        Conf::Unresolved => Confidence::Unresolved,
    }
}

fn conf_of(s: &str) -> Confidence {
    if s.starts_with("CONFIRMED") {
        Confidence::Confirmed
    } else if s.starts_with("DERIVED") {
        Confidence::Derived
    } else {
        Confidence::Unresolved
    }
}

fn icon(i: Option<Image>) -> (Image, bool) {
    match i {
        Some(i) => (i, true),
        None => (Image::default(), false),
    }
}

/// What the toast's button does.
#[derive(Clone, Default)]
enum ToastAct {
    #[default]
    None,
    Redo,
    Restore(PathBuf, String),
    Update,
    /// Read the save again keeping the staged edits (it changed on disk).
    Reload,
}

/// Toast with an optional second line and button; it stays longer when it has one.
fn toast_full(ui: &AppWindow, msg: impl Into<SharedString>, sub: &str, action: &str, act: ToastAct, error: bool) {
    let api = ui.global::<Api>();
    api.set_toast(msg.into());
    api.set_toast_sub(sub.into());
    api.set_toast_action(action.into());
    api.set_toast_error(error);
    view(|v| {
        v.toast_act = act;
        v.toast_seq += 1;
    });
    let seq = view(|v| v.toast_seq);
    let w = ui.as_weak();
    let ms = if !action.is_empty() { 9000 } else if error { 6000 } else { 3000 };
    slint::Timer::single_shot(std::time::Duration::from_millis(ms), move || {
        if let Some(ui) = w.upgrade()
            && view(|v| v.toast_seq) == seq
        {
            ui.global::<Api>().set_toast("".into());
        }
    });
}

fn toast(ui: &AppWindow, msg: impl Into<SharedString>, error: bool) {
    toast_full(ui, msg, "", "", ToastAct::None, error);
}

/// "3 changes", the count of staged edits.
fn changes(n: usize) -> String {
    trn("{n} change", "{n} changes", n as i64, &[])
}

/// "12 values", the count of values staged edits set.
fn values(n: usize) -> String {
    trn("{n} value", "{n} values", n as i64, &[])
}

/// The game saved after the save was read: nothing written, Reload keeps the edits.
fn changed_on_disk(ui: &AppWindow, p: &Path) {
    let sub = trf("{} was saved again after it was opened. Nothing was written.", &[&p.display()]);
    toast_full(ui, tr("The save changed on disk"), &sub, tr("Reload"), ToastAct::Reload, true);
}

// --- page state ---------------------------------------------------------------------

#[derive(Default)]
struct View {
    item_filter: String,
    picker_filter: String,
    /// item loadout being edited (-1 none)
    loadout_sel: i32,
    equip_filter: String,
    equip_search: String,
    equip_list_search: String,
    equip_sel: i32,
    /// box slot to scroll to on the next refresh
    equip_jump: Option<i32>,
    palico_sel: i32,
    /// My Set and Palico set being edited
    set_sel: i32,
    palset_sel: i32,
    palico_search: String,
    quest_tab: usize,
    quest_search: String,
    quest_missing: bool,
    request_filter: String,
    request_search: String,
    collection: usize,
    check_search: String,
    check_missing: bool,
    /// Unlocks page: the list's row picked, the entries' search
    unlock_sel: usize,
    unlock_search: String,
    unlock_missing: bool,
    monster_filter: String,
    monster_large: bool,
    monster_missing: bool,
    db_search: String,
    db_missing: bool,
    /// monster shown in the Database (0 none: the first listed)
    db_monster: usize,
    /// quest shown in the Database (0 none: the first listed)
    db_quest: usize,
    /// item shown in the Database (0 none: the first listed)
    db_item: usize,
    /// skill tree shown in the Database (0 none: the first listed)
    db_skill: usize,
    /// Equipment tab kind (equip_kind) and the piece shown (0 none: the first listed)
    db_equip_kind: usize,
    db_equip: usize,
    /// carve rank shown (low, high, g; the monster's highest when it has not this one)
    db_rank: String,
    field_filter: String,
    field_sel: i32,
    toast_act: ToastAct,
    toast_seq: u32,
    /// key of the field a Review entry opened, until it is shown
    goto: Option<String>,
    jump_seq: i32,
    /// the user chose to quit with staged changes
    quitting: bool,
    /// a newer release, once checked
    update: Option<update::Release>,
    /// the updated file, to restart on
    update_exe: Option<PathBuf>,
    /// set to stop the download running
    update_cancel: Option<Arc<AtomicBool>>,
    /// the Overview's goals as planned for (save state, character)
    goal_plans: Option<((u64, usize), Vec<GoalView>)>,
}

/// What the Overview shows of a goal's plan.
#[derive(Clone)]
struct GoalView {
    title: String,
    summary: String,
    count: String,
    blocked: bool,
    empty: bool,
}

thread_local! {
    static VIEW: std::cell::RefCell<View> = std::cell::RefCell::new(View {
        equip_filter: "all".into(), request_filter: "open".into(), equip_sel: -1, palico_sel: -1, field_sel: -1, loadout_sel: -1, ..Default::default()
    });
}

fn view<R>(f: impl FnOnce(&mut View) -> R) -> R {
    VIEW.with_borrow_mut(f)
}

pub fn quitting() -> bool {
    view(|v| v.quitting)
}

fn quest_tabs() -> Vec<String> {
    let mut v: Vec<String> = vec![];
    for q in Char::real_quests(true) {
        if !v.contains(&q.category) {
            v.push(q.category.clone());
        }
    }
    v
}

/// Scroll the current page's list to `row` and flash `key`. A list made after the jump
/// (its page just opened) takes it on creation; after 1.8 s it is stale and dropped.
fn jump(ui: &AppWindow, row: Option<usize>, key: &str) {
    let api = ui.global::<Api>();
    let seq = view(|v| {
        v.jump_seq += 1;
        v.jump_seq
    });
    api.set_jump_row(row.map_or(-1, |r| r as i32));
    api.set_jump_seq(seq);
    api.set_highlight(key.into());
    let w = ui.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(1800), move || {
        if let Some(ui) = w.upgrade()
            && view(|v| v.jump_seq) == seq
        {
            let api = ui.global::<Api>();
            api.set_highlight("".into());
            api.set_jump_row(-1);
        }
    });
}

// --- refresh ----------------------------------------------------------------------------

pub fn refresh(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let Some(doc) = st.doc.as_ref() else {
        api.set_loaded(false);
        return;
    };
    let s = &doc.save;
    api.set_loaded(true);
    api.set_file_path(doc.loc.opened.display().to_string().into());
    let emu = system::emulator_name(&doc.loc.opened);
    api.set_path_label(system::known_emulator(&doc.loc.opened).unwrap_or("").into());
    api.set_path_tail(fmt::path_tail(&doc.loc.opened, 4).into());
    // a running emulator names itself (check-emulator); otherwise the save's folder does
    if !api.get_emulator_running() {
        api.set_emulator_name(emu.into());
    }
    api.set_slot(st.slot as i32);
    let slots: Vec<SlotInfo> = (0..3)
        .map(|k| {
            let c = character::get(s, s.base(k));
            SlotInfo { slot: k as i32, used: s.slot_used(k), name: c.name.into(), hr: c.hr as i32, playtime: fmt::playtime(c.playtime).into() }
        })
        .collect();
    api.set_other_characters(model(slots.iter().filter(|x| x.used && x.slot as usize != st.slot).cloned().collect()));
    api.set_slots(model(slots));
    review(ui, st);
    // paths elided in the middle, so the part that differs stays visible (02.4)
    api.set_write_targets(strings(doc.loc.copies.iter().map(|p| fmt::elide_path(&p.display().to_string(), 64))));
    let warn: Vec<String> = doc
        .copies
        .iter()
        .filter(|(_, c)| *c != store::CopyState::Same)
        .map(|(p, c)| format!("{}: {:?}", p.display(), c))
        .collect();
    api.set_warning(if warn.is_empty() { "".into() } else { trf("These copies differ from the opened file and will be overwritten: {}", &[&warn.join("; ")]).into() });

    if !s.slot_used(st.slot) {
        return;
    }
    match api.get_page().as_str() {
        "overview" => overview(ui, st),
        "character" => character_page(ui, st),
        "items" => items_page(ui, st),
        "equipment" => equipment_page(ui, st),
        "palicoes" => palico_page(ui, st),
        "quests" => quests_page(ui, st),
        "requests" => requests_page(ui, st),
        "collections" => collections_page(ui, st),
        "unlocks" => unlocks_page(ui, st),
        "monsters" => monsters_page(ui, st),
        "database" => database_page(ui, st),
        "advanced" => fields_page(ui, st),
        _ => {}
    }
    // a Review entry asked for this field: scroll to it now that its page is built
    if let Some(key) = view(|v| v.goto.take()) {
        let row = goto_row(ui, &key);
        let t = find_target(st, &key);
        jump(ui, row, &t.map(|t| t.row_key()).unwrap_or(key));
    }
}

/// Review panel and Write dialog (mockup A): every value old → new, grouped by page.
fn review(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let doc = st.doc.as_ref().unwrap();
    let staged = st.staged();
    let mut counts = vec![0i32; PAGES.len()];
    for (slot, t) in &staged {
        if *slot == st.slot {
            counts[targets::page_index(t.page())] += 1;
        }
    }
    api.set_page_counts(model(counts));
    api.set_value_count(staged.len() as i32);
    api.set_change_count(st.ops.len() as i32);
    api.set_review_summary(
        if st.ops.is_empty() {
            tr("Nothing staged").to_string()
        } else {
            trf("{} · {} · not written yet", &[&changes(st.ops.len()), &values(staged.len())])
        }
        .into(),
    );
    let label = |t: &Target, slot: usize| t.label(&doc.save, slot);
    // what Write would put in the save, not the value right after the edit
    let warn = |t: &Target, slot: usize| crate::warnings::of(*t, &doc.save, slot).unwrap_or_default();
    let warn_count = |o: &crate::state::Op| o.values.iter().filter(|(t, _)| !warn(t, o.slot).is_empty()).count();
    let op_warning = |o: &crate::state::Op| match (o.values.len(), warn_count(o)) {
        (_, 0) => String::new(),
        (1, _) => warn(&o.values[0].0, o.slot),
        (_, n) => trn("{n} value the game cannot produce", "{n} values the game cannot produce", n as i64, &[]),
    };
    let page_of = |o: &crate::state::Op| o.values.first().map(|(t, _)| targets::page_index(t.page())).unwrap_or(0);
    let mut ops: Vec<&crate::state::Op> = st.ops.iter().rev().collect();
    // grouped by page in nav order, then by character; newest first within a group
    ops.sort_by_key(|o| (o.slot != st.slot, o.slot, page_of(o)));
    let mut rows = vec![];
    let mut last_group = String::new();
    for o in ops {
        let pi = page_of(o);
        let page = targets::page_title(pi);
        let group = if o.slot != st.slot { trf("Character {} · {}", &[&(o.slot + 1), &page]) } else { page.to_string() };
        let first = group != last_group;
        last_group = group.clone();
        let single = o.values.len() == 1;
        let (old, new) = match o.values.first() {
            Some((t, v)) if single => (t.read(&doc.orig, o.slot), v.clone()),
            _ => (String::new(), String::new()),
        };
        let lines: Vec<ValueLine> = if single {
            vec![]
        } else {
            o.values
                .iter()
                .take(8)
                .map(|(t, v)| ValueLine { label: label(t, o.slot).into(), old: t.read(&doc.orig, o.slot).into(), new: v.into(), warning: warn(t, o.slot).into() })
                .collect()
        };
        let sub = if single {
            o.note.clone()
        } else {
            let mut s = vec![];
            if !o.detail.is_empty() {
                s.push(o.detail.clone());
            }
            s.push(values(o.values.len()));
            s.join(" · ")
        };
        rows.push(ChangeRow {
            id: o.id,
            group: group.into(),
            first,
            title: if single { label(&o.values[0].0, o.slot) } else { o.title.clone() }.into(),
            sub: sub.into(),
            confidence: conf(o.conf),
            single,
            old: old.into(),
            new: new.into(),
            lines: model(lines),
            more: o.values.len().saturating_sub(if single { 1 } else { 8 }) as i32,
            note: if single { String::new() } else { o.note.clone() }.into(),
            key: o.values.first().map(|(t, _)| format!("{}|{}", o.slot, t.key())).unwrap_or_default().into(),
            warning: op_warning(o).into(),
        });
    }
    api.set_changes(model(rows));
    // Write dialog: changes first, then the checks, then the files (02.3)
    let n_values = staged.len();
    let mut chars: Vec<usize> = st.ops.iter().map(|o| o.slot).collect();
    chars.sort_unstable();
    chars.dedup();
    let who: Vec<String> = chars.iter().map(|&k| trf("character {}, {}", &[&(k + 1), &character::get(&doc.save, doc.save.base(k)).name])).collect();
    api.set_write_title(trn("Write {n} change to the save", "Write {n} changes to the save", st.ops.len() as i64, &[]).into());
    api.set_write_sub(format!("{} · {}", values(n_values), who.join("; ")).into());
    api.set_write_rows(model(
        st.ops
            .iter()
            .map(|o| {
                let single = o.values.len() == 1;
                WriteRow {
                    title: if single { label(&o.values[0].0, o.slot) } else { o.title.clone() }.into(),
                    right: if single { String::new() } else { values(o.values.len()) }.into(),
                    confidence: conf(o.conf),
                    old: if single { o.values[0].0.read(&doc.orig, o.slot) } else { String::new() }.into(),
                    new: if single { o.values[0].1.clone() } else { String::new() }.into(),
                    warning: op_warning(o).into(),
                }
            })
            .collect(),
    ));
    api.set_derived_count(st.ops.iter().filter(|o| o.conf != Conf::Confirmed).count() as i32);
    api.set_warn_count(staged.iter().filter(|(slot, t)| !warn(t, *slot).is_empty()).count() as i32);
    api.set_warn_why(crate::warnings::why().into());
}

/// Confirmed only (Settings) refuses edits not checked in game, and says so.
pub(crate) fn refused(ui: &AppWindow, c: Conf) -> bool {
    let r = c != Conf::Confirmed && settings::get().confirmed_only;
    if r {
        toast_full(ui, tr("Not changed: this change is Derived"), tr("Confirmed changes only is on in Settings"), "", ToastAct::None, true);
    }
    r
}

// --- actions --------------------------------------------------------------------------

/// The staged target with this key (current character first).
fn find_target(st: &State, key: &str) -> Option<Target> {
    let mut ops: Vec<&crate::state::Op> = st.ops.iter().collect();
    ops.sort_by_key(|o| o.slot != st.slot);
    ops.iter().flat_map(|o| o.values.iter()).map(|(t, _)| *t).find(|t| t.key() == key)
}

/// Show the field of a Review entry: its page, a view that lists it (01.6).
fn goto(ui: &AppWindow, st: &State, key: &str) {
    if let Some(t) = find_target(st, key) {
        goto_target(ui, st, t, key);
    }
}

/// Show field `t` on its page, flashing `key`'s row.
fn goto_target(ui: &AppWindow, st: &State, t: Target, key: &str) {
    let api = ui.global::<Api>();
    view(|v| match t {
        Target::Item(s, _) => {
            v.item_filter.clear();
            api.set_item_filter("".into());
            api.set_item_store(if s == Store::Pouch { 1 } else { 0 });
        }
        Target::Equip(o, i) => {
            api.set_equip_owner(if o == Owner::Palico { 1 } else { 0 });
            v.equip_filter = "all".into();
            v.equip_list_search.clear();
            api.set_equip_filter("all".into());
            api.set_equip_search("".into());
            v.equip_sel = i as i32;
        }
        Target::Palico(i, _) => v.palico_sel = i as i32,
        Target::MySet(k, _) => {
            api.set_equip_owner(2);
            v.set_sel = k as i32;
        }
        Target::PalicoSet(k) => {
            api.set_equip_owner(3);
            v.palset_sel = k as i32;
        }
        Target::Quest(i) => {
            if let Some(q) = tables().quests.iter().find(|q| q.index == i) {
                v.quest_tab = quest_tabs().iter().position(|c| *c == q.category).unwrap_or(0);
            }
            v.quest_search.clear();
            v.quest_missing = false;
            api.set_quest_search("".into());
            api.set_quest_missing(false);
        }
        // the Arena records tab, after the quest categories
        Target::Arena(_) => v.quest_tab = quest_tabs().len(),
        Target::Request(_) => {
            v.request_filter = "all".into();
            v.request_search.clear();
            api.set_request_filter("all".into());
            api.set_request_search("".into());
        }
        Target::Art(_) | Target::Dish(_) | Target::Ingredient(_) | Target::Award(_) | Target::Permits(_) | Target::PermitPoints(..) | Target::Levels(_) => {
            v.collection = match t {
                Target::Art(_) => 0,
                Target::Dish(_) => 1,
                Target::Ingredient(_) => 2,
                Target::Award(_) => 3,
                _ => 4,
            };
            api.set_collection_tab(v.collection as i32);
            v.check_search.clear();
            v.check_missing = false;
            api.set_check_search("".into());
            api.set_check_missing(false);
        }
        Target::Unlock(_) | Target::Pet(_) | Target::Moofahs | Target::MoofahGifts => {
            let rows = unlock_rows();
            v.unlock_sel = match t {
                Target::Unlock(m) => rows.iter().position(|r| r.maps.contains(&m)).unwrap_or(0),
                _ => rows.iter().position(|r| r.maps.is_empty()).unwrap_or(0),
            };
            v.unlock_search.clear();
            v.unlock_missing = false;
            api.set_unlock_search("".into());
            api.set_unlock_missing(false);
        }
        Target::Monster(..) => {
            v.monster_filter.clear();
            v.monster_large = false;
            v.monster_missing = false;
            api.set_monster_filter("".into());
            api.set_monster_large_only(false);
            api.set_monster_missing(false);
        }
        _ => {}
    });
    view(|v| v.goto = Some(key.to_string()));
    api.set_review_open(api.get_review_open() && ui.window().size().width as f32 / ui.window().scale_factor() >= 1440.0);
    if api.get_page().as_str() == t.page() {
        refresh(ui, st);
    } else {
        // page-shown refreshes the new page, which then takes the goto
        api.set_page(t.page().into());
    }
}

/// Row of the field `key` in its page's current model.
fn goto_row(ui: &AppWindow, key: &str) -> Option<usize> {
    let api = ui.global::<Api>();
    use slint::Model;
    let kind = key.split(':').next()?;
    // the row id: the monster index ("mon:48:hunts"), else the last field ("item:box:12")
    let n = if kind == "mon" { key.split(':').nth(1) } else { key.rsplit(':').next() }?;
    let n = n.parse::<i32>().ok()?;
    match kind {
        "item" => api.get_item_slots().iter().position(|r| r.slot == n),
        "equip" => api.get_equip_rows().iter().position(|r| r.slot == n),
        "quest" => api.get_quests().iter().position(|r| !r.header && r.index == n),
        "request" => api.get_requests().iter().position(|r| r.index == n),
        "check" => api.get_checks().iter().position(|r| r.key == n),
        "mon" => api.get_monsters().iter().position(|r| r.index == n),
        _ => None,
    }
}

/// Wire a callback that edits the save, then refresh the UI.
macro_rules! on {
    ($ui:expr, $st:expr, $name:ident, |$w:ident, $s:ident $(, $a:ident : $t:ty)*| $body:block) => {{
        let w: Weak<AppWindow> = $ui.as_weak();
        let st = $st.clone();
        $ui.global::<Api>().$name(move |$($a: $t),*| {
            let Some($w) = w.upgrade() else { return Default::default() };
            let r = {
                #[allow(unused_mut)]
                let mut $s = st.borrow_mut();
                $body
            };
            refresh(&$w, &st.borrow());
            r
        });
    }};
}

/// The full id of a page's bulk action: the store or tab it applies to.
fn bulk_id(ui: &AppWindow, id: &str) -> String {
    let api = ui.global::<Api>();
    match id {
        "items:sort" | "items:max" | "items:empty" => format!("{id}:{}", if store_of(ui) == Store::Pouch { "pouch" } else { "box" }),
        "checks:all" | "checks:none" => format!("{id}:{}", api.get_collection_tab()),
        "unlocks:all" | "unlocks:none" => format!("{id}:{}", api.get_unlock_sel()),
        _ => id.to_string(),
    }
}

/// Run a goal or bulk action as one edit; only values that change are listed.
fn apply_plan(ui: &AppWindow, s: &mut State, id: &str) {
    let slot = s.slot;
    let p = goals::plan(id, s.save(), slot);
    if p.lines.is_empty() {
        return;
    }
    let goal = goals::GOALS.iter().find(|g| g.id == id);
    let e = Edit {
        key: if goal.is_some() { format!("goal:{id}") } else { String::new() },
        title: p.review.clone(),
        detail: p.detail.clone(),
        note: p.note.clone(),
        conf: p.conf.unwrap_or(Conf::Confirmed),
        targets: vec![],
    };
    if refused(ui, e.conf) {
        return;
    }
    let id2 = id.to_string();
    s.edit(e, |sv, _| {
        let pre = sv.clone();
        goals::apply(&id2, sv, slot).into_iter().filter(|t| t.read(&pre, slot) != t.read(sv, slot)).collect()
    });
    toast_full(ui, tr("Added to Review"), &p.review, "", ToastAct::None, false);
}

pub fn wire(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    detected(ui);
    wire_settings(ui, st);
    wire_update(ui, st);
    wire_file(ui, st);
    wire_characters(ui, st);
    wire_overview(ui, st);
    wire_character(ui, st);
    wire_items(ui, st);
    wire_equipment(ui, st);
    wire_sets(ui, st);
    wire_palicoes(ui, st);
    wire_quests(ui, st);
    wire_requests(ui, st);
    wire_collections(ui, st);
    wire_unlocks(ui, st);
    wire_monsters(ui, st);
    wire_database(ui, st);
    wire_advanced(ui, st);

    // page switches refresh their model (the window calls this when Api.page changes)
    {
        let w = ui.as_weak();
        let st2 = st.clone();
        api.on_page_shown(move || {
            if let Some(ui) = w.upgrade() {
                crate::nav::shown(&ui);
                refresh(&ui, &st2.borrow());
            }
        });
    }

    on!(ui, st, on_select_slot, |ui, s, k: i32| {
        if s.doc.is_some() && (0..3).contains(&k) {
            s.slot = k as usize;
        }
        view(|v| {
            v.equip_sel = -1;
            v.palico_sel = -1;
        });
        let _ = &ui;
    });
    on!(ui, st, on_undo, |ui, s, id: i32| {
        s.undo(id);
        let _ = &ui;
    });
    // Ctrl+Z: the latest change, said in a toast
    on!(ui, st, on_undo_last, |ui, s| {
        let Some(op) = s.ops.last() else { return };
        let (id, title) = (op.id, op.title.clone());
        s.undo(id);
        toast(&ui, trf("Undid {}", &[&title]), false);
    });
    on!(ui, st, on_undo_value, |ui, s, key: SharedString| {
        s.undo_value(&key);
        let _ = &ui;
    });
    // Undo all says what it did and offers Redo (S10)
    on!(ui, st, on_undo_all, |ui, s| {
        let n = s.undo_all();
        if n > 0 {
            toast_full(&ui, trn("Undid {n} change", "Undid {n} changes", n as i64, &[]), "", tr("Redo"), ToastAct::Redo, false);
        }
    });
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_toast_act(move || {
            let Some(ui) = w.upgrade() else { return };
            let act = view(|v| std::mem::take(&mut v.toast_act));
            ui.global::<Api>().set_toast("".into());
            match act {
                ToastAct::Redo => {
                    let n = st.borrow_mut().redo_all();
                    refresh(&ui, &st.borrow());
                    if n > 0 {
                        toast(&ui, trn("Redid {n} change", "Redid {n} changes", n as i64, &[]), false);
                    }
                }
                ToastAct::Restore(dir, when) => {
                    let api = ui.global::<Api>();
                    list_snapshots(&ui, &st.borrow());
                    api.set_restore_ask(dir.display().to_string().into());
                    api.set_restore_when(when.into());
                    api.invoke_check_emulator();
                    api.set_snapshots_open(true);
                }
                ToastAct::Update => ui.global::<Api>().set_update_open(true),
                ToastAct::Reload => {
                    let r = st.borrow_mut().reload_keep();
                    refresh(&ui, &st.borrow());
                    match r {
                        Ok(n) => toast_full(&ui, tr("Read the save again"), &trn("{n} staged change kept", "{n} staged changes kept", n as i64, &[]), "", ToastAct::None, false),
                        Err(e) => toast(&ui, trf("Could not read the save again: {}", &[&e]), true),
                    }
                }
                ToastAct::None => {}
            }
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        // "<slot>|<key>" from Review: an entry of another character switches to it first
        api.on_goto(move |key| {
            let Some(ui) = w.upgrade() else { return };
            let (slot, key) = match key.split_once('|') {
                Some((s, k)) => (s.parse::<usize>().ok(), k),
                None => (None, key.as_str()),
            };
            let used = |k: usize| k < 3 && st.borrow().doc.as_ref().is_some_and(|d| d.save.slot_used(k));
            if let Some(k) = slot.filter(|&k| used(k) && k != st.borrow().slot) {
                st.borrow_mut().slot = k;
                view(|v| {
                    v.equip_sel = -1;
                    v.palico_sel = -1;
                });
            }
            goto(&ui, &st.borrow(), key);
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_check_emulator(move || {
            if let Some(ui) = w.upgrade() {
                let running = system::running_emulators();
                let api = ui.global::<Api>();
                api.set_emulator_running(!running.is_empty());
                if let Some(n) = running.first() {
                    let ryujinx = n.to_lowercase().starts_with("ryujinx") || n.to_lowercase().starts_with("ryubing");
                    api.set_emulator_name(if ryujinx { "Ryujinx".to_string() } else { n.clone() }.into());
                } else if let Some(d) = st.borrow().doc.as_ref() {
                    api.set_emulator_name(system::emulator_name(&d.loc.opened).into());
                }
            }
        });
    }
}

// One module per page (its models and callbacks), declared after `on!` so they can use
// it; what they share stays here.
// (`*_page` where the name is taken by the save library's module)
mod advanced;
mod character_page;
mod characters;
mod collections;
mod database;
mod equipment_page;
mod file;
mod items_page;
mod monsters_page;
mod names;
mod overview;
mod palicoes;
mod quests;
mod requests;
mod sets_ui;
mod settings_ui;
mod unlocks_ui;
mod update_ui;

use self::{
    advanced::*, character_page::*, characters::*, collections::*, database::*, equipment_page::*, items_page::*, monsters_page::*, names::*, overview::*,
    palicoes::*, quests::*, requests::*, sets_ui::*, settings_ui::*, unlocks_ui::*,
};
pub use self::names::{armor_parts, deco_slots, deco_used, equip_value, piece, weapon_classes};
pub use self::sets_ui::{arts_value, box_piece_value, palico_piece_label, pigment_value, set_piece_label};
pub use self::unlocks_ui::{housekeeper_name, pet_costume_name, start_place_name, unlock_map_name, unlock_rows};
pub use self::palicoes::look_label;
pub use self::{file::*, update_ui::*};
