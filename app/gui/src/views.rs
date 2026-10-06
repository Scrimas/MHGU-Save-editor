//! Models for each page and the callbacks that edit the save.

use crate::assets;
use crate::fmt::{self, count, num};
use crate::goals;
use crate::settings;
use crate::state::{Conf, Edit, State};
use crate::system;
use crate::targets::{self, Mon, Target, PAGES};
use crate::{model, strings, Shared};
use crate::{
    Api, AppWindow, ArtRow, ChangeRow, CharacterInfo, CheckRow, Confidence, DecoRow, DetectedSave, DeviantRow, EquipDetail,
    EquipRow, FieldRow, Goal, ItemSlot, LoadoutRow, MonsterRow, PalicoDetail, PalicoRow, PickItem, Preview, PreviewLine,
    QuestRow, RequestRow, SettingsInfo, SlotInfo, SnapRow, StatCard, ValueLine, WriteRow,
};
use mhgu_save::data::tables;
use mhgu_save::equipment::{self, Kind, Owner};
use mhgu_save::items::{self, Stack, Store};
use mhgu_save::progress::{Char, Lock, QuestBit, DEVIANTS};
use mhgu_save::{character, monsters, palico, store};
use slint::{ComponentHandle, Image, Model, SharedString, Weak};
use std::path::{Path, PathBuf};

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
        if let Some(ui) = w.upgrade() {
            if view(|v| v.toast_seq) == seq {
                ui.global::<Api>().set_toast("".into());
            }
        }
    });
}

fn toast(ui: &AppWindow, msg: impl Into<SharedString>, error: bool) {
    toast_full(ui, msg, "", "", ToastAct::None, error);
}

// --- names ----------------------------------------------------------------------------

/// Box type 7 + NN, NN = the game's weaponNN tables (no 05).
const WEAPON_CLASSES: [&str; 15] = [
    "Great Sword", "Sword and Shield", "Hammer", "Lance", "Heavy Bowgun", "Weapon", "Light Bowgun", "Long Sword",
    "Switch Axe", "Gunlance", "Bow", "Dual Blades", "Hunting Horn", "Insect Glaive", "Charge Blade",
];
const ARMOR_PARTS: [&str; 5] = ["Head", "Chest", "Arms", "Waist", "Legs"];

fn kind_label(k: Kind) -> String {
    match k {
        Kind::Empty => "Empty".into(),
        Kind::Head | Kind::Chest | Kind::Arms | Kind::Waist | Kind::Legs => ARMOR_PARTS[k.code() as usize - 1].into(),
        Kind::Talisman => "Talisman".into(),
        Kind::Weapon(w) => WEAPON_CLASSES.get(w as usize).copied().unwrap_or("Weapon").into(),
        // the Palico box holds types 22-24 only (docs/11)
        Kind::Other(22) => "Palico weapon".into(),
        Kind::Other(23) => "Palico head".into(),
        Kind::Other(24) => "Palico body".into(),
        Kind::Other(c) => format!("Type {c}"),
    }
}

/// The game's table of `k` pieces for `owner` (empty without the asset pack).
fn pieces(owner: Owner, k: Kind) -> &'static [assets::Piece] {
    let n = assets::names();
    match (owner, k) {
        (Owner::Hunter, Kind::Weapon(w)) => n.weapons.get(&w.to_string()),
        (Owner::Hunter, Kind::Talisman) => Some(&n.talismans),
        (Owner::Hunter, k) if k.is_armor() => n.armor.get(&k.code().to_string()),
        (Owner::Palico, Kind::Weapon(_)) => Some(&n.palico_weapons),
        (Owner::Palico, k) if k.is_armor() => n.palico_armor.get(&k.code().to_string()),
        _ => None,
    }
    .map_or(&[], Vec::as_slice)
}

fn piece(owner: Owner, k: Kind, id: u16) -> Option<&'static assets::Piece> {
    pieces(owner, k).iter().find(|p| p.id == id as u32)
}

/// What the equipment picker offers (hunter box): the weapon classes of the asset pack,
/// the armor parts and talismans.
fn equip_categories() -> Vec<Kind> {
    let mut v: Vec<Kind> = (0..WEAPON_CLASSES.len() as u8).map(Kind::Weapon).collect();
    v.extend([Kind::Head, Kind::Chest, Kind::Arms, Kind::Waist, Kind::Legs, Kind::Talisman]);
    v.retain(|&k| !pieces(Owner::Hunter, k).is_empty());
    v
}

fn equip_keep(filter: &str, k: Kind) -> bool {
    match filter {
        "weapon" => matches!(k, Kind::Weapon(_) | Kind::Other(22)),
        "armor" => k.is_armor() || matches!(k, Kind::Other(23) | Kind::Other(24)),
        "talisman" => k == Kind::Talisman,
        "empty" => k == Kind::Empty,
        _ => k != Kind::Empty,
    }
}

fn equip_name(owner: Owner, e: &equipment::Entry) -> String {
    let k = e.kind();
    match k {
        Kind::Empty => "Empty".into(),
        Kind::Talisman => assets::names()
            .talismans
            .get(e.id() as usize)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| tier_name(e.talisman().unwrap().tier).to_string()),
        _ => piece(owner, k, e.id())
            .map(|p| p.name_at(e.level() as u32).to_string())
            .unwrap_or_else(|| format!("{} #{}", kind_label(k), e.id())),
    }
}

/// "Fire Res +1 · Attack +3 · 2 slots": what tells talismans apart (10.2).
fn talisman_skills(e: &equipment::Entry) -> String {
    let Some(t) = e.talisman() else { return String::new() };
    let mut d: Vec<String> = (0..2).filter(|&j| t.skills[j] != 0).map(|j| format!("{} {:+}", skill_name(t.skills[j]), t.points[j])).collect();
    if d.is_empty() {
        d.push("No skills".into());
    }
    d.push(count(t.slots as usize, "slot", "slots"));
    d.join(" · ")
}

/// A box entry as a value in Review: "Elder Rod Lv 3", "Fire Res +1 · 0 slots", "Empty".
pub fn equip_value(owner: Owner, e: &equipment::Entry) -> String {
    match e.kind() {
        Kind::Empty => "Empty".into(),
        Kind::Talisman => talisman_skills(e),
        _ => format!("{} Lv {}", equip_name(owner, e), e.level()),
    }
}

fn piece_rarity(owner: Owner, e: &equipment::Entry) -> u32 {
    match e.kind() {
        Kind::Talisman => assets::names().talismans.get(e.id() as usize).map(|p| p.rarity).unwrap_or(0),
        k => piece(owner, k, e.id()).map(|p| p.rarity).unwrap_or(0),
    }
}

fn tier_name(t: u8) -> &'static str {
    match t {
        97 => "Mystery Talisman",
        98 => "Shining Talisman",
        99 => "Timeworn Talisman",
        100 => "Enduring Talisman",
        _ => "Talisman",
    }
}

fn skill_name(id: u8) -> String {
    if id == 0 {
        return "—".into();
    }
    assets::names().skills.get(id as usize).cloned().unwrap_or_else(|| format!("Skill #{id}"))
}

// --- page state ---------------------------------------------------------------------

#[derive(Default)]
struct View {
    item_filter: String,
    picker_filter: String,
    equip_filter: String,
    equip_search: String,
    equip_list_search: String,
    equip_sel: i32,
    /// box slot to scroll to on the next refresh
    equip_jump: Option<i32>,
    palico_sel: i32,
    quest_tab: usize,
    quest_search: String,
    quest_missing: bool,
    request_filter: String,
    request_search: String,
    collection: usize,
    check_search: String,
    check_missing: bool,
    monster_filter: String,
    monster_large: bool,
    monster_missing: bool,
    field_filter: String,
    field_sel: i32,
    toast_act: ToastAct,
    toast_seq: u32,
    /// key of the field a Review entry opened, until it is shown
    goto: Option<String>,
    jump_seq: i32,
    /// the user chose to quit with staged changes
    quitting: bool,
}

thread_local! {
    static VIEW: std::cell::RefCell<View> = std::cell::RefCell::new(View {
        equip_filter: "all".into(), request_filter: "open".into(), equip_sel: -1, palico_sel: -1, field_sel: -1, ..Default::default()
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
        if let Some(ui) = w.upgrade() {
            if view(|v| v.jump_seq) == seq {
                let api = ui.global::<Api>();
                api.set_highlight("".into());
                api.set_jump_row(-1);
            }
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
    api.set_path_label(if emu == "Ryujinx" { emu } else { "" }.into());
    api.set_path_tail(fmt::path_tail(&doc.loc.opened, 4).into());
    // a running emulator names itself (check-emulator); otherwise the save's folder does
    if !api.get_emulator_running() {
        api.set_emulator_name(emu.into());
    }
    api.set_slot(st.slot as i32);
    api.set_slots(model(
        (0..3)
            .map(|k| {
                let c = character::get(s, s.base(k));
                SlotInfo { slot: k as i32, used: s.slot_used(k), name: c.name.into(), hr: c.hr as i32, playtime: fmt::playtime(c.playtime).into() }
            })
            .collect(),
    ));
    review(ui, st);
    // paths elided in the middle, so the part that differs stays visible (02.4)
    api.set_write_targets(strings(doc.loc.copies.iter().map(|p| fmt::elide_path(&p.display().to_string(), 64))));
    let warn: Vec<String> = doc
        .copies
        .iter()
        .filter(|(_, c)| *c != store::CopyState::Same)
        .map(|(p, c)| format!("{}: {:?}", p.display(), c))
        .collect();
    api.set_warning(if warn.is_empty() { "".into() } else { format!("These copies differ from the opened file and will be overwritten: {}", warn.join("; ")).into() });

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
        "monsters" => monsters_page(ui, st),
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
            "Nothing staged".to_string()
        } else {
            format!("{} · {} · not written yet", count(st.ops.len(), "change", "changes"), count(staged.len(), "value", "values"))
        }
        .into(),
    );
    let label = |t: &Target, slot: usize| t.label(&doc.save, slot);
    let page_of = |o: &crate::state::Op| o.values.first().map(|(t, _)| targets::page_index(t.page())).unwrap_or(0);
    let mut ops: Vec<&crate::state::Op> = st.ops.iter().rev().collect();
    // grouped by page in nav order, then by character; newest first within a group
    ops.sort_by_key(|o| (o.slot != st.slot, o.slot, page_of(o)));
    let mut rows = vec![];
    let mut last_group = String::new();
    for o in ops {
        let pi = page_of(o);
        let group = if o.slot != st.slot { format!("Character {} · {}", o.slot + 1, PAGES[pi].1) } else { PAGES[pi].1.to_string() };
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
            o.values.iter().take(8).map(|(t, v)| ValueLine { label: label(t, o.slot).into(), old: t.read(&doc.orig, o.slot).into(), new: v.into() }).collect()
        };
        let sub = if single {
            o.note.clone()
        } else {
            let mut s = vec![];
            if !o.detail.is_empty() {
                s.push(o.detail.clone());
            }
            s.push(count(o.values.len(), "value", "values"));
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
            key: o.values.first().map(|(t, _)| t.key()).unwrap_or_default().into(),
        });
    }
    api.set_changes(model(rows));
    // Write dialog: changes first, then the checks, then the files (02.3)
    let values = staged.len();
    let mut chars: Vec<usize> = st.ops.iter().map(|o| o.slot).collect();
    chars.sort_unstable();
    chars.dedup();
    let who: Vec<String> = chars.iter().map(|&k| format!("character {}, {}", k + 1, character::get(&doc.save, doc.save.base(k)).name)).collect();
    api.set_write_title(format!("Write {} to the save", count(st.ops.len(), "change", "changes")).into());
    api.set_write_sub(format!("{} · {}", count(values, "value", "values"), who.join("; ")).into());
    api.set_write_rows(model(
        st.ops
            .iter()
            .map(|o| {
                let single = o.values.len() == 1;
                WriteRow {
                    title: if single { label(&o.values[0].0, o.slot) } else { o.title.clone() }.into(),
                    right: if single { String::new() } else { count(o.values.len(), "value", "values") }.into(),
                    confidence: conf(o.conf),
                    old: if single { o.values[0].0.read(&doc.orig, o.slot) } else { String::new() }.into(),
                    new: if single { o.values[0].1.clone() } else { String::new() }.into(),
                }
            })
            .collect(),
    ));
    api.set_derived_count(st.ops.iter().filter(|o| o.conf != Conf::Confirmed).count() as i32);
}

fn overview(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let mut s2 = s.clone();
    let c = Char::new(&mut s2, st.slot);
    let t = tables();
    let real = Char::real_quests(true);
    let done = real.iter().filter(|q| c.quest(QuestBit::Cleared, q.index)).count();
    let arts = t.arts.iter().filter(|a| c.art(a.0)).count();
    let dishes = (0..99).filter(|&i| c.dish(i)).count();
    // the same total as Collections: every award of the card (07.1)
    let missing: Vec<String> = t.awards.iter().filter(|a| !c.award(a.0)).map(|a| a.2.clone()).collect();
    let listed: Vec<usize> = (1..=monsters::N).filter(|&i| monsters::meta(i).crown_awards).collect();
    let met = listed.iter().filter(|&&i| {
        let r = monsters::get(s, base, i);
        r.hunts + r.captures > 0
    }).count();
    let gold = listed.iter().filter(|&&i| monsters::crowns(i, monsters::get(s, base, i)).large == 2).count();
    let mini = listed.iter().filter(|&&i| monsters::crowns(i, monsters::get(s, base, i)).mini).count();
    let chs = character::get(s, base);
    let card = |title: &str, n: usize, of: usize, sub: String, page: &str, tab: i32| StatCard {
        title: title.into(),
        value: format!("{} / {}", num(n as i64), num(of as i64)).into(),
        sub: sub.into(),
        progress: if of == 0 { 0.0 } else { n as f32 / of as f32 },
        done: n >= of && of > 0,
        bar: n < of,
        page: page.into(),
        tab,
    };
    let awards_sub = match missing.len() {
        0 => String::new(),
        1 => format!("1 missing: {}", missing[0]),
        n => format!("{n} missing"),
    };
    api.set_stats(model(vec![
        card("Quests cleared", done, real.len(), format!("Village ★{} · Hub ★{}", c.village_star(), c.hub_star()), "quests", 0),
        card("Hunter Arts", arts, t.arts.len(), String::new(), "collections", 0),
        card("Canteen dishes", dishes, 99, String::new(), "collections", 1),
        card("Awards", t.awards.len() - missing.len(), t.awards.len(), awards_sub, "collections", 3),
        // what is counted is named, so 79 here and 93 on Monsters can both be right (07.2)
        card("Large monsters met", met, listed.len(), "Guild Card list".into(), "monsters", 0),
        card("Gold crowns", gold, listed.len(), format!("{} mini crowns", num(mini as i64)), "monsters", 0),
        // values, not progress: no bars (H1.2)
        StatCard {
            title: "Hunter Rank".into(),
            value: num(chs.hr).into(),
            sub: if chs.hr >= 999 { format!("Max · {} HR points", num(chs.hr_points)) } else { format!("{} HR points", num(chs.hr_points)) }.into(),
            page: "character".into(),
            ..Default::default()
        },
        StatCard {
            title: "Zenny".into(),
            value: num(chs.funds).into(),
            sub: if chs.funds >= character::MAX_FUNDS { "Max" } else { "" }.into(),
            page: "character".into(),
            ..Default::default()
        },
    ]));
    let mut cards = vec![];
    let mut done_goals = vec![];
    for g in &goals::GOALS {
        let staged = st.ops.iter().find(|o| o.slot == st.slot && o.key == format!("goal:{}", g.id));
        let p = goals::plan(g.id, s, st.slot);
        let goal = |state: i32, detail: String, cnt: String, op: i32| Goal {
            id: g.id.into(),
            title: p.title.clone().into(),
            detail: detail.into(),
            count: cnt.into(),
            confidence: conf(g.conf),
            state,
            op,
        };
        match staged {
            Some(o) => cards.push(goal(1, "Its changes are in Review. Nothing is written until you press Write.".into(), String::new(), o.id)),
            None if p.blocked => cards.push(goal(3, p.summary.clone(), p.count.clone(), 0)),
            None if p.lines.is_empty() => done_goals.push(goal(2, p.summary.clone(), String::new(), 0)),
            None => cards.push(goal(0, p.summary.clone(), p.count.clone(), 0)),
        }
    }
    api.set_goals(model(cards));
    api.set_goals_done(model(done_goals));
}

fn character_page(ui: &AppWindow, st: &State) {
    let s = st.save();
    let c = character::get(s, st.base());
    let mut s2 = s.clone();
    let p = Char::new(&mut s2, st.slot);
    let was = |t: Target| -> SharedString { st.was(t).into() };
    ui.global::<Api>().set_character(CharacterInfo {
        name: c.name.into(),
        hr: c.hr as i32,
        hr_points: c.hr_points as i32,
        funds: c.funds as i32,
        wycademy: c.wycademy as i32,
        playtime_h: (c.playtime / 3600) as i32,
        playtime_m: (c.playtime / 60 % 60) as i32,
        village_star: p.village_star() as i32,
        hub_star: p.hub_star() as i32,
        gender: match c.gender {
            0 => "Type 1 (male)".into(),
            1 => "Type 2 (female)".into(),
            g => format!("{g}").into(),
        },
        points_lr: model(c.points_lr.iter().map(|&v| v as i32).collect()),
        points_g: model(c.points_g.iter().map(|&v| v as i32).collect()),
        was_name: was(Target::Name),
        was_hr: was(Target::Hr),
        was_hr_points: was(Target::HrPoints),
        was_funds: was(Target::Funds),
        was_wycademy: was(Target::Wycademy),
        was_playtime: was(Target::Playtime),
        was_village_star: was(Target::VillageStar),
        was_hub_star: was(Target::HubStar),
        was_lr: model((0..4).map(|v| was(Target::Points(v, false))).collect()),
        was_g: model((0..4).map(|v| was(Target::Points(v, true))).collect()),
    });
}

fn store_of(ui: &AppWindow) -> Store {
    if ui.global::<Api>().get_item_store() == 1 { Store::Pouch } else { Store::Box }
}

fn items_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let store = store_of(ui);
    let all = items::all(s, base, store);
    let f = view(|v| v.item_filter.to_lowercase());
    let off = if store == Store::Box { items::BOX } else { items::POUCH };
    let rows: Vec<ItemSlot> = all
        .iter()
        .enumerate()
        .filter(|(_, x)| f.is_empty() || (!x.is_empty() && assets::item_name(x.id).to_lowercase().contains(&f)))
        .map(|(i, x)| {
            let (img, has) = if x.is_empty() { (Image::default(), false) } else { icon(assets::item_icon(x.id)) };
            let bit = 19 * i;
            let changed = st.changed(base + off + bit / 8, 3) && !st.was(Target::Item(store, i)).is_empty();
            ItemSlot {
                slot: i as i32,
                id: x.id as i32,
                name: if x.is_empty() { "".into() } else { assets::item_name(x.id).into() },
                count: x.count as i32,
                max: if x.is_empty() { 0 } else { assets::item_max(x.id, store) as i32 },
                icon: img,
                has_icon: has,
                changed,
                was: if changed { st.was(Target::Item(store, i)) } else { String::new() }.into(),
            }
        })
        .collect();
    api.set_item_used(all.iter().filter(|x| !x.is_empty()).count() as i32);
    api.set_item_total(all.len() as i32);
    api.set_item_slots(model(rows));
    // runs of empty loadouts collapse into one row (C8)
    let mut lrows: Vec<LoadoutRow> = vec![];
    let mut run: Option<(usize, usize)> = None;
    let flush = |run: &mut Option<(usize, usize)>, lrows: &mut Vec<LoadoutRow>| {
        if let Some((a, b)) = run.take() {
            let name = if a == b { format!("Loadout {} · empty", a + 1) } else { format!("Loadouts {}–{} · empty", a + 1, b + 1) };
            lrows.push(LoadoutRow { index: -1, name: name.into(), summary: "".into(), used: false });
        }
    };
    for k in 0..items::LOADOUT_N {
        let l = items::loadout(s, base, k);
        let used: Vec<String> = l.items.iter().filter(|x| x.0 != 0).map(|x| format!("{} ×{}", assets::item_name(x.0), x.1)).collect();
        if used.is_empty() {
            run = Some(run.map_or((k, k), |(a, _)| (a, k)));
            continue;
        }
        flush(&mut run, &mut lrows);
        lrows.push(LoadoutRow { index: k as i32, name: l.name.into(), used: true, summary: used.join(", ").into() });
    }
    flush(&mut run, &mut lrows);
    api.set_loadouts(model(lrows));
    picker(ui);
}

fn picker(ui: &AppWindow) {
    let f = view(|v| v.picker_filter.to_lowercase());
    let n = assets::names();
    let store = store_of(ui);
    let mut v: Vec<PickItem> = vec![];
    let count = if n.items.is_empty() { 1900 } else { n.items.len() };
    for id in 1..count.min(items::MAX_ID as usize + 1) {
        let name = assets::item_name(id as u16);
        if n.items.get(id).is_some_and(|x| x.is_empty() || x == "(None)" || x.starts_with('-')) {
            continue;
        }
        if !f.is_empty() && !name.to_lowercase().contains(&f) && id.to_string() != f {
            continue;
        }
        let (img, has) = icon(assets::item_icon(id as u16));
        let max = assets::item_max(id as u16, store) as i32;
        v.push(PickItem { id: id as i32, name: name.into(), icon: img, has_icon: has, sub: SharedString::default(), max });
        if v.len() >= 400 {
            break;
        }
    }
    ui.global::<Api>().set_pick_items(model(v));
}

fn owner_of(ui: &AppWindow) -> Owner {
    if ui.global::<Api>().get_equip_owner() == 1 { Owner::Palico } else { Owner::Hunter }
}

/// "your current gear, My Set 2 “Fire DB”" for the references to hunter box entry `i`.
fn uses_label(s: &mhgu_save::Save, base: usize, i: usize) -> String {
    equipment::uses(s, base, i)
        .into_iter()
        .map(|u| match u {
            equipment::Use::Worn => "your current gear".to_string(),
            equipment::Use::MySet(n, name) => format!("My Set {n} “{name}”"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The pieces of the selected category matching the picker search.
fn equip_picker(ui: &AppWindow) {
    let api = ui.global::<Api>();
    let Some(&k) = equip_categories().get(api.get_equip_category().max(0) as usize) else {
        api.set_pick_equip(model(vec![]));
        return;
    };
    let f = view(|v| v.equip_search.to_lowercase());
    let v: Vec<PickItem> = pieces(Owner::Hunter, k)
        .iter()
        .filter(|p| p.is_real())
        .filter(|p| f.is_empty() || p.id.to_string() == f || [&p.name].into_iter().chain(&p.names).any(|n| n.to_lowercase().contains(&f)))
        .map(|p| {
            let mut sub = vec![format!("Rare {}", p.rarity)];
            match k {
                Kind::Talisman => sub.push(tier_name(equipment::talisman_tier(p.id as u16)).into()),
                // upgrade names past the max level and the limit break
                Kind::Weapon(_) if p.names.len() == 3 && p.names[1] != p.name => sub.push(format!("then {}", p.names[1..].join(", "))),
                _ => {}
            }
            match (p.male, p.female) {
                (Some(1), Some(0)) => sub.push("type 1 (male) only".into()),
                (Some(0), Some(1)) => sub.push("type 2 (female) only".into()),
                _ => {}
            }
            let (img, has) = icon(assets::equip_icon(k.code(), p.rarity));
            PickItem { id: p.id as i32, name: p.name.clone().into(), icon: img, has_icon: has, sub: sub.join(" · ").into(), max: 0 }
        })
        .collect();
    api.set_pick_equip(model(v));
}

fn equipment_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let orig = st.orig();
    let base = st.base();
    let owner = owner_of(ui);
    let off = if owner == Owner::Hunter { equipment::BOX } else { equipment::PALICO_BOX };
    let (f, q) = view(|v| (v.equip_filter.clone(), v.equip_list_search.to_lowercase()));
    let mut used = 0;
    let rows: Vec<EquipRow> = (0..owner.len())
        .filter_map(|i| {
            let e = equipment::get(s, base, owner, i);
            let k = e.kind();
            used += !e.is_empty() as usize;
            if !equip_keep(&f, k) {
                return None;
            }
            let (title, sub) = match k {
                Kind::Empty => ("Empty".to_string(), String::new()),
                Kind::Talisman => (talisman_skills(&e), format!("{} · Talisman", equip_name(owner, &e))),
                // Palico gear has no level or decorations; its type only when the name doesn't say it (K3)
                _ if owner == Owner::Palico => {
                    let (name, kind) = (equip_name(owner, &e), kind_label(k));
                    let sub = if name.starts_with(&kind) { String::new() } else { kind };
                    (name, sub)
                }
                _ => {
                    let decos = e.decos().iter().filter(|&&d| d != 0).count();
                    let mut sub = vec![kind_label(k), format!("Lv {}", e.level())];
                    if decos > 0 {
                        sub.push(count(decos, "decoration", "decorations"));
                    }
                    if e.transmog() != 0 {
                        sub.push("transmog".into());
                    }
                    (equip_name(owner, &e), sub.join(" · "))
                }
            };
            // search matches the name, the weapon type and talisman skills (08.2)
            if !q.is_empty() && !title.to_lowercase().contains(&q) && !sub.to_lowercase().contains(&q) && (i + 1).to_string() != q {
                return None;
            }
            let rarity = piece_rarity(owner, &e);
            let (img, has) = icon(assets::equip_icon(k.code(), rarity));
            let changed = st.changed(base + off + equipment::ENTRY * i, equipment::ENTRY);
            let status = if !changed {
                ""
            } else {
                let was = equipment::get(orig, base, owner, i);
                if was.is_empty() && !e.is_empty() {
                    "New · in Review"
                } else if e.is_empty() {
                    "Removed · in Review"
                } else {
                    "Changed · in Review"
                }
            };
            Some(EquipRow { slot: i as i32, kind_code: k.code() as i32, title: title.into(), sub: sub.into(), icon: img, has_icon: has, changed, status: status.into() })
        })
        .collect();
    api.set_equip_summary(
        if owner == Owner::Hunter {
            format!("Hunter box: {} / {} slots used · confirmed in game except where marked", num(used as i64), num(owner.len() as i64))
        } else {
            format!("Palico box: {} / {} slots used · Palico gear names are not extracted from the game yet", num(used as i64), num(owner.len() as i64))
        }
        .into(),
    );
    // after selecting or adding, the row is scrolled into view (10.1)
    if let Some(slot) = view(|v| v.equip_jump.take()) {
        if let Some(row) = rows.iter().position(|r| r.slot == slot) {
            jump(ui, Some(row), "");
        }
    }
    api.set_equip_rows(model(rows));
    let sel = view(|v| v.equip_sel);
    let mut d = EquipDetail { slot: -1, category: -1, ..Default::default() };
    if sel >= 0 && (sel as usize) < owner.len() {
        let i = sel as usize;
        let e = equipment::get(s, base, owner, i);
        let k = e.kind();
        let ds = e.decos();
        let t = e.talisman();
        // one row per decoration slot the piece has (10.3): a talisman's slot count is in
        // the save; for other pieces only the decorations present are known
        let decos: Vec<DecoRow> = match t {
            Some(t) => (0..(t.slots as usize).min(3)).map(|j| DecoRow { index: j as i32, name: if ds[j] == 0 { "".into() } else { assets::item_name(ds[j]).into() } }).collect(),
            None => (0..3).filter(|&j| ds[j] != 0).map(|j| DecoRow { index: j as i32, name: assets::item_name(ds[j]).into() }).collect(),
        };
        let field = tables()
            .fields
            .iter()
            .find(|fl| fl.block == "char1" && fl.rel <= off && off < fl.rel + fl.size)
            .map(|fl| fl.label.clone())
            .unwrap_or_default();
        let was_e = equipment::get(orig, base, owner, i);
        let staged = st.changed(base + off + equipment::ENTRY * i, equipment::ENTRY);
        d = EquipDetail {
            slot: sel,
            kind: kind_label(k).into(),
            kind_code: k.code() as i32,
            name: if k == Kind::Talisman { talisman_skills(&e) } else { equip_name(owner, &e) }.into(),
            id: e.id() as i32,
            level: e.level() as i32,
            was_level: if staged && was_e.kind() == k && was_e.id() == e.id() && was_e.level() != e.level() { num(was_e.level()) } else { String::new() }.into(),
            decos: model(decos),
            transmog: if e.transmog() == 0 {
                "".into()
            } else {
                piece(owner, k, e.transmog()).map(|p| p.name.clone()).unwrap_or_else(|| format!("#{}", e.transmog())).into()
            },
            transmog_id: e.transmog() as i32,
            is_talisman: t.is_some(),
            skill1: t.map(|t| t.skills[0] as i32).unwrap_or(0),
            points1: t.map(|t| t.points[0] as i32).unwrap_or(0),
            skill2: t.map(|t| t.skills[1] as i32).unwrap_or(0),
            points2: t.map(|t| t.points[1] as i32).unwrap_or(0),
            slots: t.map(|t| t.slots as i32).unwrap_or(0),
            tier: if k == Kind::Talisman { format!("{} · {}", equip_name(owner, &e), tier_name(t.unwrap().tier)) } else { String::new() }.into(),
            raw: e.raw.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ").into(),
            field: field.into(),
            uses: if owner == Owner::Hunter { uses_label(s, base, i) } else { String::new() }.into(),
            category: equip_categories().iter().position(|&c| c == k).map_or(-1, |p| p as i32),
            was: if staged { equip_value(owner, &was_e) } else { String::new() }.into(),
        };
    }
    api.set_equip_detail(d);
    let cats = equip_categories();
    api.set_equip_categories(strings(cats.iter().map(|&k| kind_label(k))));
    api.set_equip_free(match equipment::free_slot(s, base, Owner::Hunter) {
        Some(i) if !cats.is_empty() => i as i32,
        _ => -1,
    });
    let skills: Vec<String> = if assets::names().skills.is_empty() {
        (0..160).map(|i| if i == 0 { "—".to_string() } else { format!("Skill #{i}") }).collect()
    } else {
        assets::names().skills.iter().enumerate().map(|(i, n)| if i == 0 { "—".into() } else { n.clone() }).collect()
    };
    api.set_skill_names(strings(skills));
}

fn palico_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let rows: Vec<PalicoRow> = (0..palico::LIST_N)
        .filter(|&i| !palico::is_empty(s, base, i))
        .map(|i| {
            let p = palico::get(s, base, i);
            PalicoRow {
                index: i as i32,
                name: p.name.into(),
                sub: format!("Lv {} · {}", p.level, palico::BIASES.get(p.bias as usize).copied().unwrap_or("?")).into(),
                changed: st.changed(base + palico::LIST + palico::RECORD * i, palico::RECORD),
            }
        })
        .collect();
    let mut sel = view(|v| v.palico_sel);
    if sel < 0 {
        sel = rows.first().map(|r| r.index).unwrap_or(-1);
        view(|v| v.palico_sel = sel);
    }
    api.set_palico_summary(count(rows.len(), "Palico", "Palicoes").into());
    api.set_palicoes(model(rows));
    api.set_biases(strings(palico::BIASES.iter().map(|s| s.to_string())));
    let mv = |m: &[u8]| {
        let n = &assets::names().support_moves;
        // 0 is "(No Move)", 57 an empty learned slot
        let v: Vec<String> = m.iter().filter(|&&x| x != 0 && x != palico::NO_MOVE && x != 0xFF).map(|&x| n.get(x as usize).cloned().unwrap_or_else(|| format!("#{x}"))).collect();
        if v.is_empty() { "None".to_string() } else { v.join(", ") }
    };
    api.set_palico(if sel >= 0 {
        let i = sel as usize;
        let p = palico::get(s, base, i);
        let was = |f: targets::Pal| -> SharedString { st.was(Target::Palico(i, f)).into() };
        PalicoDetail {
            index: sel,
            name: p.name.into(),
            level: p.level as i32,
            exp: p.exp as i32,
            bias: p.bias as i32,
            target: p.target as i32,
            greeting: p.greeting.into(),
            owner: p.owner.into(),
            moves: mv(&p.moves).into(),
            learned: mv(&p.learned).into(),
            was_name: was(targets::Pal::Name),
            was_level: was(targets::Pal::Level),
            was_exp: was(targets::Pal::Exp),
            was_bias: was(targets::Pal::Bias),
            was_greeting: was(targets::Pal::Greeting),
            was_owner: was(targets::Pal::Owner),
        }
    } else {
        PalicoDetail { index: -1, ..Default::default() }
    });
}

fn lock_text(l: &Lock) -> (String, bool) {
    match l {
        Lock::Unlocked => (String::new(), false),
        Lock::Rotated => ("listed only in its rotation".into(), false),
        Lock::Event => ("event quest: listed once downloaded".into(), false),
        Lock::Locked(alts) => (format!("locked: needs {}", alts.iter().map(|a| a.join(" and ")).collect::<Vec<_>>().join(", or ")), true),
    }
}

/// Deviant of a Special Permit quest index.
fn deviant_of(index: usize) -> Option<usize> {
    (0..DEVIANTS.len()).find(|&d| {
        let (q0, n) = Char::deviant_levels(d);
        (q0..q0 + n).contains(&index)
    })
}

/// Group of a quest on its tab: rank, or the deviant on Special Permit (R9).
fn quest_group(q: &mhgu_save::data::Quest) -> String {
    match deviant_of(q.index) {
        Some(d) if q.category == "Special Permit" => format!("dev:{d}"),
        _ => format!("rank:{}", q.rank),
    }
}

fn quests_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let mut s2 = st.save().clone();
    let c = Char::new(&mut s2, st.slot);
    let tabs = quest_tabs();
    let tab = view(|v| v.quest_tab).min(tabs.len().saturating_sub(1));
    let (search, missing) = view(|v| (v.quest_search.to_lowercase(), v.quest_missing));
    let cat = tabs.get(tab).cloned().unwrap_or_default();
    let mut qs: Vec<_> = Char::real_quests(true).into_iter().filter(|q| q.category == cat).collect();
    qs.sort_by_key(|q| (deviant_of(q.index).unwrap_or(0), q.rank.parse::<u32>().unwrap_or(99), q.id));
    let base = st.base();
    let total = qs.len();
    let done = qs.iter().filter(|q| c.quest(QuestBit::Cleared, q.index)).count();
    let mut rows = vec![];
    let mut group = String::from("\0");
    for q in &qs {
        let cl = c.quest(QuestBit::Cleared, q.index);
        if (missing && cl) || (!search.is_empty() && !q.name.to_lowercase().contains(&search) && !q.id.to_string().contains(&search)) {
            continue;
        }
        let g = quest_group(q);
        if g != group {
            group = g.clone();
            let members: Vec<_> = qs.iter().filter(|x| quest_group(x) == g).collect();
            let open = members.iter().any(|x| !c.quest(QuestBit::Cleared, x.index));
            let (label, action) = match deviant_of(q.index) {
                Some(d) if cat == "Special Permit" => (DEVIANTS[d].to_string(), "Mark all cleared".to_string()),
                _ if q.rank.is_empty() => (cat.clone(), "Mark all cleared".to_string()),
                _ => (format!("{cat} {}★", q.rank), format!("Mark {}★ cleared", q.rank)),
            };
            let cleared = members.iter().filter(|x| c.quest(QuestBit::Cleared, x.index)).count();
            rows.push(QuestRow {
                header: true,
                name: format!("{label} · {} / {}", cleared, members.len()).into(),
                group: g.into(),
                action: if open { action } else { String::new() }.into(),
                ..Default::default()
            });
        }
        let (lock, locked) = if cl { (String::new(), false) } else { lock_text(&c.lock(q.id)) };
        let byte = |o: usize| base + o + q.index / 8;
        let changed = [mhgu_save::progress::CLEARED, mhgu_save::progress::SEEN, mhgu_save::progress::FAILED].iter().any(|&o| st.changed(byte(o), 1));
        rows.push(QuestRow {
            index: q.index as i32,
            id: q.id as i32,
            name: q.name.clone().into(),
            sub: format!("#{}{}", q.id, if lock.is_empty() { String::new() } else { format!(" · {lock}") }).into(),
            cleared: cl,
            seen: c.quest(QuestBit::Seen, q.index),
            failed: c.quest(QuestBit::Failed, q.index),
            locked,
            prowler: q.prowler,
            changed,
            was: if changed { st.was(Target::Quest(q.index)) } else { String::new() }.into(),
            header: false,
            group: SharedString::default(),
            action: SharedString::default(),
        });
    }
    api.set_quest_summary(format!("{cat}: {} / {} cleared · confirmed in game except where marked", num(done as i64), num(total as i64)).into());
    api.set_quest_tabs(strings(tabs));
    api.set_quest_tab(tab as i32);
    api.set_quests(model(rows));
}

fn requests_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let mut s2 = st.save().clone();
    let c = Char::new(&mut s2, st.slot);
    let (f, q) = view(|v| (v.request_filter.clone(), v.request_search.to_lowercase()));
    let base = st.base();
    let (mut open, mut done_n) = (0, 0);
    let rows: Vec<RequestRow> = tables()
        .requests
        .iter()
        .filter_map(|r| {
            let acc = r.accept_flag.is_some_and(|x| c.flag(x));
            let done = r.done_flag.is_some_and(|x| c.flag(x));
            if done { done_n += 1 } else { open += 1 }
            let keep = match f.as_str() {
                "open" => !done,
                "done" => done,
                _ => true,
            };
            let village = match r.village.as_str() {
                "Bherna" | "Kokoto" | "Pokke" | "Yukumo" => r.village.clone(),
                _ => "Hub".to_string(),
            };
            let name = if r.quest_name.is_empty() { "Delivery request".into() } else { r.quest_name.clone() };
            if !keep || (!q.is_empty() && !name.to_lowercase().contains(&q) && !village.to_lowercase().contains(&q)) {
                return None;
            }
            let waiting = if acc { String::new() } else { c.offer_missing(r.index).join("; ") };
            let flag_changed = |x: Option<usize>| x.is_some_and(|x| st.changed(base + mhgu_save::progress::FLAGS + x / 8, 1));
            let changed = flag_changed(r.accept_flag) || flag_changed(r.done_flag);
            Some(RequestRow {
                index: r.index as i32,
                name: name.into(),
                sub: format!("{village} · #{}{}", r.index, if waiting.is_empty() { String::new() } else { format!(" · waits for: {waiting}") }).into(),
                accepted: acc,
                completed: done,
                has_flags: r.accept_flag.is_some(),
                changed,
                was: if changed { st.was(Target::Request(r.index)) } else { String::new() }.into(),
            })
        })
        .collect();
    api.set_request_summary(format!("{} open · {} completed", num(open), num(done_n)).into());
    api.set_requests(model(rows));
}

/// Base name of a Hunter Art level ("Ground Slash III" -> "Ground Slash").
fn art_base(name: &str) -> &str {
    for suffix in [" III", " II", " I"] {
        if let Some(b) = name.strip_suffix(suffix) {
            return b;
        }
    }
    name
}

fn collections_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let mut s2 = st.save().clone();
    let c = Char::new(&mut s2, st.slot);
    let tab = view(|v| v.collection);
    let (q, missing) = view(|v| (v.check_search.to_lowercase(), v.check_missing));
    let t = tables();
    let keep = |name: &str, on: bool| !(missing && on) && (q.is_empty() || name.to_lowercase().contains(&q));
    let changed_of = |tg: Target| !st.was(tg).is_empty();
    // counts in each tab (H5.1)
    let arts_on = t.arts.iter().filter(|a| c.art(a.0)).count();
    let dishes_on = (0..99).filter(|&b| c.dish(b)).count();
    let ingr_on = (0..45).filter(|&b| c.ingredient(b)).count();
    let awards_on = t.awards.iter().filter(|a| c.award(a.0)).count();
    api.set_collection_tabs(strings([
        format!("Hunter Arts {} / {}", arts_on, t.arts.len()),
        format!("Canteen dishes {dishes_on} / 99"),
        format!("Canteen ingredients {ingr_on} / 45"),
        format!("Awards {} / {}", awards_on, t.awards.len()),
        "Deviants".to_string(),
    ]));
    let rows: Vec<CheckRow> = match tab {
        1 | 2 => t
            .canteen
            .iter()
            .filter(|(k, ..)| k == if tab == 1 { "dish" } else { "ingredient" })
            .map(|(_, b, n, _)| {
                let on = if tab == 1 { c.dish(*b) } else { c.ingredient(*b) };
                let tg = if tab == 1 { Target::Dish(*b) } else { Target::Ingredient(*b) };
                CheckRow { key: *b as i32, name: n.clone().into(), on, changed: changed_of(tg), ..Default::default() }
            })
            .filter(|r| keep(&r.name, r.on))
            .collect(),
        3 => t
            .awards
            .iter()
            .filter(|(b, _, n)| keep(n, c.award(*b)))
            .map(|(b, _, n)| {
                let (img, has) = icon(assets::award_icon(*b));
                CheckRow { key: *b as i32, name: n.clone().into(), on: c.award(*b), changed: changed_of(Target::Award(*b)), icon: img, has_icon: has }
            })
            .collect(),
        _ => vec![],
    };
    // Hunter Arts grouped by art: I, II and III on one row (H5.3)
    let mut arts: Vec<(String, Vec<u32>)> = vec![];
    for (id, n) in &t.arts {
        let b = art_base(n);
        match arts.last_mut() {
            Some(last) if last.0 == b && n != b => last.1.push(*id),
            _ => arts.push((b.to_string(), vec![*id])),
        }
    }
    let art_rows: Vec<ArtRow> = arts
        .into_iter()
        .filter(|(n, ids)| keep(n, ids.iter().all(|&i| c.art(i))))
        .map(|(n, ids)| ArtRow {
            name: n.into(),
            on: model(ids.iter().map(|&i| c.art(i)).collect()),
            changed: ids.iter().any(|&i| changed_of(Target::Art(i))),
            ids: model(ids.iter().map(|&i| i as i32).collect()),
        })
        .collect();
    let summary = match tab {
        0 => format!("Hunter Arts: {} / {}", arts_on, t.arts.len()),
        1 => format!("Canteen dishes: {dishes_on} / 99"),
        2 => format!("Canteen ingredients: {ingr_on} / 45"),
        3 => format!("Awards: {} / {}", awards_on, t.awards.len()),
        _ => "18 deviants".into(),
    };
    api.set_checks_summary(format!("{summary} · confirmed in game except where marked").into());
    api.set_checks(model(rows));
    api.set_arts(model(art_rows));
    let devs: Vec<DeviantRow> = DEVIANTS
        .iter()
        .enumerate()
        .map(|(d, name)| {
            let (q0, n) = Char::deviant_levels(d);
            let lv = (0..n).filter(|&k| c.quest(QuestBit::Cleared, q0 + k)).count();
            let mi = t.monsters.iter().find(|m| m.name == *name).map(|m| m.index);
            let (img, has) = icon(mi.and_then(assets::monster_icon));
            DeviantRow {
                index: d as i32,
                name: (*name).into(),
                permits: c.permits(d) as i32,
                levels: lv as i32,
                total: n as i32,
                icon: img,
                has_icon: has,
                was_permits: st.was(Target::Permits(d)).into(),
                was_levels: st.was(Target::Levels(d)).into(),
            }
        })
        .collect();
    api.set_deviants(model(devs));
}

/// Large monster that can still get a crown.
fn misses_crown(i: usize, r: monsters::Record) -> bool {
    let c = monsters::crowns(i, r);
    monsters::meta(i).size_record && monsters::meta(i).family_of.is_none() && (!c.mini && !monsters::meta(i).fixed_size || c.large < 2)
}

fn monsters_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let (f, large, missing) = view(|v| (v.monster_filter.to_lowercase(), v.monster_large, v.monster_missing));
    let t = tables();
    let mut n_large = 0;
    let mut listed = 0;
    let was = |i: usize, m: Mon| -> SharedString { st.was(Target::Monster(i, m)).into() };
    let rows: Vec<MonsterRow> = t
        .monsters
        .iter()
        .filter(|m| !m.name.is_empty() && !m.name.starts_with("dummy"))
        .filter(|m| (106..=112).contains(&m.index).then_some(false).unwrap_or(true))
        .filter_map(|m| {
            let meta = monsters::meta(m.index);
            let r = monsters::get(s, base, m.index);
            n_large += m.large as usize;
            listed += meta.crown_awards as usize;
            if (large && !m.large) || (missing && !misses_crown(m.index, r)) {
                return None;
            }
            if !f.is_empty() && !m.name.to_lowercase().contains(&f) {
                return None;
            }
            let (img, has) = icon(assets::monster_icon(m.index));
            let crown = monsters::crowns(m.index, r);
            let mut sub = vec![format!("#{}", m.index)];
            if !meta.class.is_empty() {
                sub.push(meta.class.clone());
            } else if !m.large {
                sub.push("small monster".into());
            }
            if let Some(h) = meta.family_of {
                sub.push(format!("size kept by {}", t.monsters[h - 1].name));
            } else if let Some(b) = meta.base_cm.filter(|_| meta.size_record && r.max > 0 && r.min > 0) {
                sub.push(format!("{:.0}–{:.0} cm", b * r.min as f32 / 100.0, b * r.max as f32 / 100.0));
            }
            let thresholds = if meta.fixed_size {
                "fixed size: any record is gold".to_string()
            } else {
                format!("mini ≤ {} % · silver ≥ {} % · gold ≥ {} %", meta.mini_le, meta.silver_ge, meta.gold_ge)
            };
            let notes = monsters::notes(s, base, m.index);
            let changed = st.changed(base + monsters::HUNTS + 2 * m.index, 2)
                || st.changed(base + monsters::CAPTURES + 2 * m.index, 2)
                || st.changed(base + monsters::SIZES + 4 * m.index, 4)
                || meta.notes_bit.is_some_and(|b| st.changed(base + monsters::NOTES + b / 8, 1));
            let w = |f: Mon| if changed { was(m.index, f) } else { SharedString::default() };
            Some(MonsterRow {
                index: m.index as i32,
                name: m.name.clone().into(),
                icon: img,
                has_icon: has,
                large: m.large,
                has_size: meta.size_record,
                sub: sub.join(" · ").into(),
                hunts: r.hunts as i32,
                captures: r.captures as i32,
                min: r.min as i32,
                max: r.max as i32,
                mini: crown.mini,
                crown: crown.large as i32,
                notes: notes.unwrap_or(false),
                has_notes: notes.is_some(),
                thresholds: thresholds.into(),
                was_hunts: w(Mon::Hunts),
                was_captures: w(Mon::Captures),
                was_min: w(Mon::Min),
                was_max: w(Mon::Max),
                was_notes: w(Mon::Notes),
                changed,
            })
        })
        .collect();
    api.set_monster_summary(
        format!("{} large monsters, {} of them on the Guild Card list · {} shown · confirmed in game except where marked", n_large, listed, rows.len()).into(),
    );
    api.set_monsters(model(rows));
}

fn fields_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let f = view(|v| v.field_filter.to_lowercase());
    let fields = &tables().fields;
    let abs = |fl: &mhgu_save::data::Field| if fl.block == "char1" { base + fl.rel } else { fl.abs };
    let rows: Vec<FieldRow> = fields
        .iter()
        .enumerate()
        .filter(|(_, fl)| {
            f.is_empty() || fl.label.to_lowercase().contains(&f) || fl.manager.to_lowercase().contains(&f) || format!("{:x}", abs(fl)).contains(f.trim_start_matches("0x"))
        })
        .map(|(i, fl)| FieldRow {
            index: i as i32,
            offset: format!("0x{:06X}", abs(fl)).into(),
            size: if fl.size >= 1024 { format!("{:.1} KB", fl.size as f32 / 1024.0) } else { format!("{} B", fl.size) }.into(),
            owner: fl.manager.clone().into(),
            label: fl.label.clone().into(),
            confidence: conf_of(&fl.confidence),
            changed: st.changed(abs(fl), fl.size.min(s.bytes().len() - abs(fl))),
        })
        .collect();
    api.set_fields(model(rows));
    let sel = view(|v| v.field_sel);
    api.set_field_sel(sel);
    if let Some(fl) = (sel >= 0).then(|| fields.get(sel as usize)).flatten() {
        let a = abs(fl);
        let n = fl.size.min(512);
        let mut hex = String::new();
        for row in (0..n).step_by(16) {
            hex.push_str(&format!("{:06X}  ", a + row));
            for k in 0..16.min(n - row) {
                let o = a + row + k;
                let mark = if st.changed(o, 1) { '*' } else { ' ' };
                hex.push_str(&format!("{:02x}{mark}", s.u8(o)));
            }
            hex.push('\n');
        }
        if fl.size > n {
            hex.push_str(&format!("… {} more bytes\n", num(fl.size as i64 - n as i64)));
        }
        api.set_hex(hex.into());
        let c = conf_of(&fl.confidence);
        let tag = match c {
            Confidence::Confirmed => "Confirmed",
            Confidence::Derived => "Derived",
            _ => "Unresolved",
        };
        api.set_field_info(
            format!(
                "{}\n{} · {} · {} · {tag}",
                fl.label,
                fl.manager,
                count(fl.size, "byte", "bytes"),
                if fl.block == "char1" { format!("base + 0x{:X}", fl.rel) } else { format!("block {}", fl.block) }
            )
            .into(),
        );
    } else {
        api.set_hex("".into());
        api.set_field_info("".into());
    }
}

/// A save on the Open screen: its characters (R12).
fn save_row(p: &Path) -> DetectedSave {
    let info = system::save_info(p);
    let (title, sub) = match &info {
        Some(i) if !i.names.is_empty() => {
            let (n, hr, t) = &i.names[0];
            let more = if i.names.len() > 1 {
                format!(" · also {}", i.names[1..].iter().map(|x| x.0.clone()).collect::<Vec<_>>().join(", "))
            } else {
                String::new()
            };
            (n.clone(), format!("HR {} · {}{more}", num(*hr), fmt::playtime(*t)))
        }
        Some(_) => ("No characters yet".to_string(), String::new()),
        None => ("Unreadable save".to_string(), String::new()),
    };
    DetectedSave {
        path: p.display().to_string().into(),
        title: title.into(),
        sub: sub.into(),
        when: info.and_then(|i| i.modified).map(fmt::when).unwrap_or_default().into(),
        short: fmt::elide_path(&p.display().to_string(), 70).into(),
    }
}

/// The Open screen's lists: recent saves still on disk, then the saves found on this
/// computer that are not among them.
pub fn detected(ui: &AppWindow) {
    let recent: Vec<PathBuf> = settings::get().recent.into_iter().filter(|p| p.is_file()).collect();
    let found: Vec<PathBuf> = system::detect_saves().into_iter().filter(|p| !recent.contains(p)).collect();
    let api = ui.global::<Api>();
    api.set_recent(model(recent.iter().map(|p| save_row(p)).collect()));
    api.set_detected(model(found.iter().map(|p| save_row(p)).collect()));
}

/// The Settings dialog's values.
fn settings_ui(ui: &AppWindow) {
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
    });
}

/// Confirmed only (Settings) refuses edits not checked in game, and says so.
fn refused(ui: &AppWindow, c: Conf) -> bool {
    let r = c != Conf::Confirmed && settings::get().confirmed_only;
    if r {
        toast_full(ui, "Not changed: this change is Derived", "Confirmed changes only is on in Settings", "", ToastAct::None, true);
    }
    r
}

/// Snapshot folder of the open save (`<root>/<save id>`).
fn snapshot_dir(st: &State) -> Option<PathBuf> {
    let doc = st.doc.as_ref()?;
    let id = doc.loc.save_dir.as_ref().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "save".into());
    Some(system::snapshot_root().join(id))
}

fn wire_settings(ui: &AppWindow) {
    let api = ui.global::<Api>();
    settings_ui(ui);
    let theme = settings::get().theme;
    if theme != "system" {
        api.set_force_scheme(theme.into());
    }
    let w = ui.as_weak();
    api.on_set_setting(move |key, v| {
        let Some(ui) = w.upgrade() else { return };
        let on = v != 0;
        let at = |list: &[u32]| list.get(v as usize).copied().unwrap_or(0);
        settings::update(|s| match key.as_str() {
            "theme" => s.theme = settings::THEMES.get(v as usize).unwrap_or(&"system").to_string(),
            "scale" => s.scale = at(&settings::SCALES),
            "keep" => s.keep_snapshots = at(&settings::KEEPS),
            "snapshot-default-dir" => s.snapshot_dir = None,
            "snapshot-first" => s.snapshot_first = on,
            "reopen" => s.reopen_last = on,
            "confirmed-only" => s.confirmed_only = on,
            _ => eprintln!("unknown setting {key:?}"),
        });
        if key == "theme" {
            ui.global::<Api>().set_force_scheme(settings::get().theme.into());
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
                s.save_folders.remove(i as usize);
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

// --- actions --------------------------------------------------------------------------

pub fn open(ui: &AppWindow, st: &Shared, p: &Path) {
    let r = st.borrow_mut().open(p);
    match r {
        Ok(()) => {
            let api = ui.global::<Api>();
            api.set_page("overview".into());
            view(|v| {
                v.equip_sel = -1;
                v.palico_sel = -1;
                v.field_sel = -1;
            });
            let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
            settings::update(|s| s.add_recent(abs));
            detected(ui);
            settings_ui(ui);
            refresh(ui, &st.borrow());
            let n = st.borrow().doc.as_ref().map(|d| (0..3).filter(|&k| d.save.slot_used(k)).count()).unwrap_or(0);
            toast(ui, format!("Save opened: {}", count(n, "character", "characters")), false);
        }
        Err(e) => {
            ui.global::<Api>().set_warning(format!("Could not open {}: {e}", p.display()).into());
            toast(ui, e, true);
        }
    }
}

/// The staged target with this key (current character first).
fn find_target(st: &State, key: &str) -> Option<Target> {
    let mut ops: Vec<&crate::state::Op> = st.ops.iter().collect();
    ops.sort_by_key(|o| o.slot != st.slot);
    ops.iter().flat_map(|o| o.values.iter()).map(|(t, _)| *t).find(|t| t.key() == key)
}

/// Show the field of a Review entry: its page, a view that lists it (01.6).
fn goto(ui: &AppWindow, st: &State, key: &str) {
    let Some(t) = find_target(st, key) else { return };
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
        Target::Quest(i) => {
            if let Some(q) = tables().quests.iter().find(|q| q.index == i) {
                v.quest_tab = quest_tabs().iter().position(|c| *c == q.category).unwrap_or(0);
            }
            v.quest_search.clear();
            v.quest_missing = false;
            api.set_quest_search("".into());
            api.set_quest_missing(false);
        }
        Target::Request(_) => {
            v.request_filter = "all".into();
            v.request_search.clear();
            api.set_request_filter("all".into());
            api.set_request_search("".into());
        }
        Target::Art(_) | Target::Dish(_) | Target::Ingredient(_) | Target::Award(_) | Target::Permits(_) | Target::Levels(_) => {
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
        // the page timer refreshes the new page, which then takes the goto
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
    toast_full(ui, "Added to Review", &p.review, "", ToastAct::None, false);
}

pub fn wire(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    detected(ui);
    wire_settings(ui);

    // page switches refresh their model
    {
        let w = ui.as_weak();
        let st2 = st.clone();
        let page = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let t = slint::Timer::default();
        t.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(100), move || {
            let Some(ui) = w.upgrade() else { return };
            let p = ui.global::<Api>().get_page().to_string();
            if *page.borrow() != p {
                *page.borrow_mut() = p;
                refresh(&ui, &st2.borrow());
            }
        });
        std::mem::forget(t);
    }

    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_open_dialog(move || {
            let Some(ui) = w.upgrade() else { return };
            let mut d = rfd::FileDialog::new().set_title("Open MHGU save (system)");
            if let Some(dir) = system::detect_saves().first().and_then(|p| p.parent()) {
                d = d.set_directory(dir);
            }
            if let Some(p) = d.pick_file() {
                open(&ui, &st, &p);
            }
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_open_path(move |p| {
            if let Some(ui) = w.upgrade() {
                open(&ui, &st, Path::new(p.as_str()));
            }
        });
    }
    {
        let st = st.clone();
        api.on_open_folder(move || {
            if let Some(d) = st.borrow().doc.as_ref().and_then(|d| d.loc.opened.parent().map(Path::to_path_buf)) {
                system::open_folder(&d);
            }
        });
    }
    {
        let w = ui.as_weak();
        api.on_copy_path(move || {
            if let Some(ui) = w.upgrade() {
                toast(&ui, "Path copied", false);
            }
        });
    }
    on!(ui, st, on_select_slot, |ui, s, k: i32| {
        if s.doc.is_some() {
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
    on!(ui, st, on_undo_value, |ui, s, key: SharedString| {
        s.undo_value(&key);
        let _ = &ui;
    });
    // Undo all says what it did and offers Redo (S10)
    on!(ui, st, on_undo_all, |ui, s| {
        let n = s.undo_all();
        if n > 0 {
            toast_full(&ui, format!("Undid {}", count(n, "change", "changes")), "", "Redo", ToastAct::Redo, false);
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
                        toast(&ui, format!("Redid {}", count(n, "change", "changes")), false);
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
                ToastAct::None => {}
            }
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_goto(move |key| {
            if let Some(ui) = w.upgrade() {
                goto(&ui, &st.borrow(), &key);
            }
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
    on!(ui, st, on_write, |ui, s, snapshot: bool| {
        let running = system::running_emulators();
        if !running.is_empty() {
            toast(&ui, format!("Close {} first", running.join(", ")), true);
            ui.global::<Api>().set_emulator_running(true);
            return false;
        }
        let titles: Vec<String> = s.ops.iter().map(|o| if o.values.len() == 1 { o.values[0].0.label(s.save(), o.slot) } else { o.title.clone() }).collect();
        let n = s.ops.len();
        let Some(doc) = s.doc.as_mut() else { return false };
        let mut kept = None;
        if snapshot {
            let id = doc.loc.save_dir.as_ref().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "save".into());
            let now = chrono::Local::now();
            let stamp = now.format(system::STAMP).to_string();
            match store::snapshot(&doc.loc, &system::snapshot_root().join(id), &stamp) {
                Ok(p) => {
                    system::write_note(&p, &format!("Before: {}", fmt::list(&titles, 3)));
                    kept = Some((p, fmt::when(now)));
                }
                Err(e) => {
                    toast(&ui, format!("Snapshot failed, nothing written: {e}"), true);
                    return false;
                }
            }
        }
        match store::write_all(&mut doc.save, &doc.loc) {
            Ok(_) => {
                s.written();
                if let Some((p, _)) = &kept {
                    system::prune_snapshots(p.parent().unwrap(), p);
                }
                // after writing: a toast with Restore (02.5, S8)
                match kept {
                    Some((p, when)) => {
                        let sub = format!("Snapshot from {} kept", when.trim_start_matches("Today, "));
                        toast_full(&ui, format!("Wrote {}", count(n, "change", "changes")), &sub, "Restore…", ToastAct::Restore(p, when), false)
                    }
                    None => toast(&ui, format!("Wrote {} (no snapshot)", count(n, "change", "changes")), false),
                }
                true
            }
            Err(e) => {
                toast(&ui, format!("Write failed: {e}"), true);
                false
            }
        }
    });
    {
        let st = st.clone();
        api.on_open_snapshots(move || {
            let dir = st
                .borrow()
                .doc
                .as_ref()
                .and_then(|d| d.loc.save_dir.as_ref().and_then(|d| d.file_name()).map(|n| system::snapshot_root().join(n)))
                .filter(|p| p.is_dir())
                .unwrap_or_else(system::snapshot_root);
            system::open_folder(&dir);
        });
    }
    on!(ui, st, on_list_snapshots, |ui, s| {
        list_snapshots(&ui, &s);
    });
    // restoring snapshots the current save first, so a restore can be undone too (A3)
    on!(ui, st, on_restore, |ui, s, dir: SharedString| {
        let running = system::running_emulators();
        if !running.is_empty() {
            toast(&ui, format!("Close {} first", running.join(", ")), true);
            return;
        }
        let dir = PathBuf::from(dir.as_str());
        let when = system::snapshots(dir.parent().unwrap_or(&dir)).into_iter().find(|x| x.dir == dir).map(|x| fmt::when(x.time)).unwrap_or_default();
        let Some(doc) = s.doc.as_ref() else { return };
        let loc = doc.loc.clone();
        let id = loc.save_dir.as_ref().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "save".into());
        let stamp = chrono::Local::now().format(system::STAMP).to_string();
        let kept = match store::snapshot(&loc, &system::snapshot_root().join(id), &stamp) {
            Ok(p) => {
                system::write_note(&p, &format!("Before restoring the snapshot from {when}"));
                p
            }
            Err(e) => return toast(&ui, format!("Snapshot failed, nothing restored: {e}"), true),
        };
        match store::restore(&loc, &dir) {
            Ok(_) => {
                // after the restore: the snapshot it came from may be among the oldest
                system::prune_snapshots(kept.parent().unwrap(), &kept);
                let opened = loc.opened.clone();
                let slot = s.slot;
                if let Err(e) = s.open(&opened) {
                    return toast(&ui, format!("Restored, but reading it back failed: {e}"), true);
                }
                if s.save().slot_used(slot) {
                    s.slot = slot;
                }
                ui.global::<Api>().set_snapshots_open(false);
                toast_full(&ui, format!("Restored the save from {when}"), "The save before restoring is kept as a snapshot", "", ToastAct::None, false);
            }
            Err(e) => toast(&ui, format!("Restore failed: {e}"), true),
        }
    });
    {
        let w = ui.as_weak();
        api.on_quit(move || {
            view(|v| v.quitting = true);
            if let Some(ui) = w.upgrade() {
                let _ = ui.hide();
            }
            let _ = slint::quit_event_loop();
        });
    }

    // overview: goals and page-level bulk actions share one preview (R3, R4)
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_open_preview(move |id| {
            let Some(ui) = w.upgrade() else { return false };
            let s = st.borrow();
            let Some(doc) = s.doc.as_ref() else { return false };
            let id = bulk_id(&ui, &id);
            let p = goals::plan(&id, &doc.save, s.slot);
            let shown = 200;
            let lines: Vec<PreviewLine> = p.lines.iter().take(shown).map(|(l, a, b)| PreviewLine { label: l.into(), old: a.into(), new: b.into() }).collect();
            let empty = p.lines.is_empty();
            ui.global::<Api>().set_preview(Preview {
                id: id.clone().into(),
                title: p.title.clone().into(),
                // a goal's card already says what it does (C6m); a page action says it here
                summary: if empty {
                    format!("Nothing to change. {}", p.summary).into()
                } else if goals::GOALS.iter().any(|g| g.id == id) {
                    format!("{} change. Nothing is written until you press Write.", count(p.lines.len(), "value", "values")).into()
                } else {
                    format!("{} {} change. Nothing is written until you press Write.", p.summary, count(p.lines.len(), "value", "values")).into()
                },
                confidence: conf(p.conf.unwrap_or(Conf::Confirmed)),
                lines: model(lines),
                more: p.lines.len().saturating_sub(shown) as i32,
                note: p.note.clone().into(),
                tech: if empty { String::new() } else { p.tech.clone() }.into(),
                empty,
                action: p.action.into(),
            });
            true
        });
    }
    on!(ui, st, on_apply_preview, |ui, s, id: SharedString| {
        apply_plan(&ui, &mut s, &id);
    });

    // character: one value per edit, titled with its name
    on!(ui, st, on_set_character, |ui, s, key: SharedString, v: i32| {
        let v = v.max(0) as u32;
        let k = key.as_str();
        let t = match k {
            "hr" => Target::Hr,
            "hr-points" => Target::HrPoints,
            "funds" => Target::Funds,
            "wycademy" => Target::Wycademy,
            "village-star" => Target::VillageStar,
            "hub-star" => Target::HubStar,
            "play-h" | "play-m" => Target::Playtime,
            _ => match k.trim_start_matches(|c: char| c.is_alphabetic()).parse::<usize>() {
                Ok(i) if i < 4 => Target::Points(i, k.starts_with('g')),
                _ => return,
            },
        };
        let mut msg = None;
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            match k {
                "hr" => {
                    if !character::set_hr(sv, base, v as u16) {
                        msg = Some("HR below 13 follows the Hub star level: edit Hub ★ instead");
                    }
                }
                "hr-points" => character::set_hr_points(sv, base, v),
                "funds" => character::set_funds(sv, base, v),
                "wycademy" => character::set_wycademy(sv, base, v),
                "village-star" => sv.set_u16(base + mhgu_save::progress::VIL_STAR, (v as u16).clamp(1, 10)),
                "hub-star" => character::set_hub_star(sv, base, v as u16),
                "play-h" | "play-m" => {
                    let cur = character::get(sv, base).playtime;
                    let (h, m) = (cur / 3600, cur / 60 % 60);
                    let secs = if k == "play-h" { v * 3600 + m * 60 } else { h * 3600 + v.min(59) * 60 };
                    character::set_playtime(sv, base, secs + cur % 60);
                }
                _ => {
                    if let Target::Points(i, g) = t {
                        character::set_village_points(sv, base, i, g, v);
                    }
                }
            }
            vec![]
        });
        if let Some(m) = msg {
            toast(&ui, m, true);
        }
    });
    on!(ui, st, on_set_name, |ui, s, name: SharedString| {
        s.edit(Edit::one(Target::Name, "Name".into(), Conf::Confirmed).note("Written to the save, the player record and the Guild Card"), |sv, base| {
            character::set_name(sv, base, &name);
            vec![]
        });
        let _ = &ui;
    });

    // items
    on!(ui, st, on_filter_items, |ui, s, t: SharedString| {
        view(|v| v.item_filter = t.to_string());
        let _ = (&ui, &s);
    });
    {
        let w = ui.as_weak();
        api.on_search_picker(move |t| {
            view(|v| v.picker_filter = t.to_string());
            if let Some(ui) = w.upgrade() {
                picker(&ui);
            }
        });
    }
    on!(ui, st, on_set_item, |ui, s, slot: i32, id: i32, count: i32| {
        let store = store_of(&ui);
        let t = Target::Item(store, slot as usize);
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            let max = assets::item_max(id as u16, store) as i32;
            items::set(sv, base, store, slot as usize, Stack { id: id as u16, count: count.clamp(0, max) as u8 });
            vec![]
        });
    });
    on!(ui, st, on_item_bulk, |ui, s, what: SharedString| {
        let id = bulk_id(&ui, &format!("items:{what}"));
        apply_plan(&ui, &mut s, &id);
    });

    // equipment
    on!(ui, st, on_filter_equip, |ui, s, f: SharedString| {
        view(|v| v.equip_filter = f.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_search_equip_list, |ui, s, t: SharedString| {
        view(|v| v.equip_list_search = t.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_select_equip, |ui, s, i: i32| {
        // the list scrolls to it when it is out of view (10.1)
        view(|v| {
            v.equip_sel = i;
            v.equip_jump = Some(i);
        });
        let _ = (&ui, &s);
    });
    on!(ui, st, on_set_equip, |ui, s, field: SharedString, v: i32| {
        let owner = owner_of(&ui);
        let i = view(|v| v.equip_sel);
        if i < 0 {
            return;
        }
        let i = i as usize;
        let f = field.as_str();
        let t = Target::Equip(owner, i);
        let title = t.label(s.save(), s.slot);
        let mut e = Edit::one(t, title, Conf::Confirmed);
        e.key = format!("{}:{f}", t.key());
        s.edit(e, |sv, base| {
            let mut e = equipment::get(sv, base, owner, i);
            match f {
                "level" => e.set_level(v as u8),
                "transmog" => e.set_transmog(0, 1),
                d if d.starts_with("deco") => e.set_deco(d[4..].parse().unwrap_or(0), v as u16),
                t if e.talisman().is_some() => {
                    let mut tl = e.talisman().unwrap();
                    match t {
                        "skill0" => tl.skills[0] = v as u8,
                        "skill1" => tl.skills[1] = v as u8,
                        "points0" => tl.points[0] = v as i8,
                        "points1" => tl.points[1] = v as i8,
                        "slots" => tl.slots = v as u8,
                        _ => {}
                    }
                    e.set_talisman(tl);
                }
                _ => {}
            }
            equipment::set(sv, base, owner, i, &e);
            vec![]
        });
    });
    {
        let w = ui.as_weak();
        api.on_search_equip(move |t| {
            view(|v| v.equip_search = t.to_string());
            if let Some(ui) = w.upgrade() {
                equip_picker(&ui);
            }
        });
    }
    // add, replace or remove a hunter box entry; never one that the worn gear or a My Set uses
    on!(ui, st, on_put_equip, |ui, s, slot: i32, id: i32| {
        if owner_of(&ui) != Owner::Hunter {
            return;
        }
        let base = s.base();
        let slot = match usize::try_from(slot).ok().or_else(|| equipment::free_slot(s.save(), base, Owner::Hunter)) {
            Some(i) if i < Owner::Hunter.len() => i,
            _ => return toast(&ui, "The equipment box is full", true),
        };
        let used = uses_label(s.save(), base, slot);
        if !used.is_empty() {
            return toast(&ui, format!("Box slot {} is used by {used}", slot + 1), true);
        }
        let api = ui.global::<Api>();
        let e = match usize::try_from(api.get_equip_category()).ok().and_then(|c| equip_categories().get(c).copied()) {
            Some(k) if id > 0 => equipment::Entry::new(k, id as u16),
            _ => equipment::Entry { raw: [0; equipment::ENTRY] },
        };
        let name = equip_name(Owner::Hunter, &e);
        let t = Target::Equip(Owner::Hunter, slot);
        let mut ed = Edit::one(t, t.label(s.save(), s.slot), Conf::Confirmed);
        ed.key = format!("{}:piece", t.key());
        if !e.is_empty() {
            ed.note = "New box entry at level 1, shaped like the game's own".into();
        }
        s.edit(ed, |sv, base| {
            equipment::set(sv, base, Owner::Hunter, slot, &e);
            vec![]
        });
        view(|v| {
            v.equip_sel = slot as i32;
            v.equip_jump = Some(slot as i32);
            if !equip_keep(&v.equip_filter, e.kind()) && !e.is_empty() {
                v.equip_filter = "all".into();
                api.set_equip_filter("all".into());
            }
            v.equip_list_search.clear();
            api.set_equip_search("".into());
        });
        if !e.is_empty() {
            toast(&ui, format!("{name} added to box slot {}", slot + 1), false);
        }
    });
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_show_in_map(move |label| {
            let Some(ui) = w.upgrade() else { return };
            let i = tables().fields.iter().position(|f| f.label == label.as_str());
            view(|v| {
                v.field_filter = String::new();
                v.field_sel = i.map_or(-1, |i| i as i32);
            });
            let api = ui.global::<Api>();
            api.set_field_filter("".into());
            api.set_page("advanced".into());
            refresh(&ui, &st.borrow());
            jump(&ui, i, "");
        });
    }

    // palicoes
    on!(ui, st, on_select_palico, |ui, s, i: i32| {
        view(|v| v.palico_sel = i);
        let _ = (&ui, &s);
    });
    on!(ui, st, on_set_palico, |ui, s, f: SharedString, v: i32| {
        let i = view(|v| v.palico_sel);
        if i < 0 {
            return;
        }
        let i = i as usize;
        let t = Target::Palico(i, targets::pal_of(&f));
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            let mut p = palico::get(sv, base, i);
            match f.as_str() {
                "level" => p.level = v.clamp(1, palico::MAX_LEVEL as i32) as u8,
                "exp" => p.exp = v.max(0) as u32,
                "bias" => p.bias = v.clamp(0, 7) as u8,
                _ => {}
            }
            palico::set(sv, base, i, &p);
            vec![]
        });
        let _ = &ui;
    });
    on!(ui, st, on_set_palico_text, |ui, s, f: SharedString, t: SharedString| {
        let i = view(|v| v.palico_sel);
        if i < 0 {
            return;
        }
        let i = i as usize;
        let tg = Target::Palico(i, targets::pal_of(&f));
        let title = tg.label(s.save(), s.slot);
        s.edit(Edit::one(tg, title, Conf::Confirmed), |sv, base| {
            let mut p = palico::get(sv, base, i);
            match f.as_str() {
                "name" => p.name = t.to_string(),
                "greeting" => p.greeting = t.to_string(),
                "owner" => p.owner = t.to_string(),
                _ => {}
            }
            palico::set(sv, base, i, &p);
            vec![]
        });
        let _ = &ui;
    });

    // quests
    on!(ui, st, on_select_quest_tab, |ui, s, i: i32| {
        view(|v| v.quest_tab = i as usize);
        let _ = (&ui, &s);
    });
    on!(ui, st, on_filter_quests, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.quest_search = api.get_quest_search().to_string();
            v.quest_missing = api.get_quest_missing();
        });
        let _ = &s;
    });
    on!(ui, st, on_set_quest, |ui, s, index: i32, bit: SharedString, on: bool| {
        let which = match bit.as_str() {
            "seen" => QuestBit::Seen,
            "failed" => QuestBit::Failed,
            _ => QuestBit::Cleared,
        };
        let slot = s.slot;
        let t = Target::Quest(index as usize);
        let title = t.label(s.save(), slot);
        let mut sets = vec![];
        let mut e = Edit::one(t, title, Conf::Confirmed);
        e.key = format!("{}:{bit}", t.key());
        s.edit(e, |sv, _| {
            let mut c = Char::new(sv, slot);
            if which == QuestBit::Cleared && on {
                sets = c.clear_quests(&[index as usize]);
            } else {
                c.set_quest(which, index as usize, on);
            }
            vec![]
        });
        if !sets.is_empty() {
            if let Some(o) = s.ops.last_mut() {
                o.note = "Also completes its quest set, as the game does".into();
            }
        }
        let _ = &ui;
    });
    on!(ui, st, on_quest_bulk, |ui, s, group: SharedString| {
        let tabs = quest_tabs();
        let cat = tabs.get(view(|v| v.quest_tab)).cloned().unwrap_or_default();
        let slot = s.slot;
        let members: Vec<usize> = Char::real_quests(true).into_iter().filter(|q| q.category == cat && quest_group(q) == group.as_str()).map(|q| q.index).collect();
        let label = match group.split_once(':') {
            Some(("dev", d)) => format!("{} quests", d.parse::<usize>().ok().and_then(|d| DEVIANTS.get(d)).copied().unwrap_or("Deviant")),
            Some((_, r)) if !r.is_empty() => format!("{cat} {r}★ quests"),
            _ => format!("{cat} quests"),
        };
        let e = Edit { key: String::new(), title: format!("{label} marked cleared"), detail: "Quests".into(), note: "Completed quest sets are recorded too, as the game does".into(), conf: Conf::Confirmed, targets: vec![] };
        s.edit(e, |sv, _| {
            let mut c = Char::new(sv, slot);
            let todo: Vec<usize> = members.iter().copied().filter(|&i| !c.quest(QuestBit::Cleared, i)).collect();
            c.clear_quests(&todo);
            todo.into_iter().map(Target::Quest).collect()
        });
        let _ = &ui;
    });

    // requests
    on!(ui, st, on_filter_requests, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.request_filter = api.get_request_filter().to_string();
            v.request_search = api.get_request_search().to_string();
        });
        let _ = &s;
    });
    on!(ui, st, on_set_request, |ui, s, index: i32, what: SharedString, on: bool| {
        if refused(&ui, Conf::Derived) {
            return;
        }
        let r = tables().requests.iter().find(|r| r.index == index as usize).unwrap().clone();
        let slot = s.slot;
        let t = Target::Request(r.index);
        let title = t.label(s.save(), slot);
        let mut e = Edit::one(t, title, Conf::Derived);
        e.key = format!("{}:{what}", t.key());
        if what == "completed" && on {
            e.note = "The villager's reward is not handed over in game".into();
        }
        s.edit(e, |sv, _| {
            let mut c = Char::new(sv, slot);
            match (what.as_str(), on) {
                ("accepted", true) => c.accept_request(r.index),
                ("accepted", false) => {
                    if let Some(f) = r.accept_flag {
                        c.set_flag(f, false)
                    }
                }
                (_, v) => {
                    if let Some(f) = r.done_flag {
                        c.set_flag(f, v);
                        if v {
                            c.accept_request(r.index);
                        }
                    }
                }
            }
            vec![]
        });
        let _ = &ui;
    });

    // collections
    on!(ui, st, on_select_collection, |ui, s, i: i32| {
        view(|v| v.collection = i as usize);
        ui.global::<Api>().set_collection_tab(i);
        let _ = &s;
    });
    on!(ui, st, on_filter_checks, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.check_search = api.get_check_search().to_string();
            v.check_missing = api.get_check_missing();
        });
        let _ = &s;
    });
    on!(ui, st, on_set_check, |ui, s, key: i32, on: bool| {
        let tab = view(|v| v.collection);
        let slot = s.slot;
        let t = match tab {
            0 => Target::Art(key as u32),
            1 => Target::Dish(key as usize),
            2 => Target::Ingredient(key as usize),
            _ => Target::Award(key as usize),
        };
        let title = t.label(s.save(), slot);
        let mut e = Edit::one(t, title, Conf::Confirmed);
        if tab == 3 {
            e.note = "Written to both of the game's award lists".into();
        }
        s.edit(e, |sv, _| {
            let mut c = Char::new(sv, slot);
            match tab {
                0 => c.set_art(key as u32, on),
                1 => c.set_dish(key as usize, on),
                2 => c.set_ingredient(key as usize, on),
                _ => c.set_award(key as usize, on),
            }
            vec![]
        });
        let _ = &ui;
    });
    on!(ui, st, on_set_deviant, |ui, s, d: i32, what: SharedString, v: i32| {
        let d = d as usize;
        let slot = s.slot;
        let t = if what == "permits" { Target::Permits(d) } else { Target::Levels(d) };
        let title = t.label(s.save(), slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, _| {
            let mut c = Char::new(sv, slot);
            if what == "permits" {
                c.set_permits(d, v.clamp(0, 99) as u8);
            } else {
                let (q0, n) = Char::deviant_levels(d);
                for k in 0..n {
                    let on = (k as i32) < v;
                    c.set_quest(QuestBit::Cleared, q0 + k, on);
                    if on {
                        c.set_quest(QuestBit::Seen, q0 + k, true);
                    }
                }
            }
            vec![]
        });
        let _ = &ui;
    });

    // monsters
    on!(ui, st, on_filter_monsters, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.monster_filter = api.get_monster_filter().to_string();
            v.monster_large = api.get_monster_large_only();
            v.monster_missing = api.get_monster_missing();
        });
        let _ = &s;
    });
    on!(ui, st, on_set_monster, |ui, s, i: i32, field: SharedString, v: i32| {
        let i = i as usize;
        let f = field.as_str();
        let m = match f {
            "hunts" => Mon::Hunts,
            "captures" => Mon::Captures,
            "min" => Mon::Min,
            "max" => Mon::Max,
            _ => Mon::Notes,
        };
        let t = Target::Monster(i, m);
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed).note("Also rebuilds the Guild Card monster log"), |sv, base| {
            if m == Mon::Notes {
                monsters::set_notes(sv, base, i, v != 0);
                return vec![];
            }
            let mut r = monsters::get(sv, base, i);
            let v16 = v.clamp(0, 9999) as u16;
            let mut more = vec![];
            match m {
                Mon::Hunts => r.hunts = v16,
                Mon::Captures => r.captures = v16,
                Mon::Min => {
                    r.min = v16;
                    if r.max < v16 {
                        r.max = v16;
                        more.push(Target::Monster(i, Mon::Max));
                    }
                }
                _ => {
                    r.max = v16;
                    if r.min == 0 || r.min > v16 {
                        r.min = v16.min(r.min.max(1));
                        more.push(Target::Monster(i, Mon::Min));
                    }
                }
            }
            monsters::set(sv, base, i, r);
            more
        });
        let _ = &ui;
    });

    // advanced
    on!(ui, st, on_filter_fields, |ui, s, f: SharedString| {
        view(|v| v.field_filter = f.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_select_field, |ui, s, i: i32| {
        view(|v| v.field_sel = i);
        let _ = (&ui, &s);
    });
}

fn list_snapshots(ui: &AppWindow, st: &State) {
    let Some(root) = snapshot_dir(st) else { return };
    let rows: Vec<SnapRow> = system::snapshots(&root)
        .into_iter()
        .map(|x| SnapRow {
            dir: x.dir.display().to_string().into(),
            when: fmt::when(x.time).into(),
            before: if x.before.is_empty() { "Before a write".to_string() } else { x.before }.into(),
        })
        .collect();
    let api = ui.global::<Api>();
    api.set_snapshots(model(rows));
    api.set_snapshot_dir(fmt::elide_path(&root.display().to_string(), 56).into());
}
