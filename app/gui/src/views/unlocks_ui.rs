//! The Unlocks page: the character's other unlock maps (`mhgu_save::unlocks`), entry by
//! entry, and the village pets.

use super::*;
use crate::{PetRow, UnlockCell, UnlockRow};
use mhgu_save::data::UnlockEntry;
use mhgu_save::unlocks::{self, Kind, Refusal};

/// A row of the page's list: the maps it shows (several for the Armory), none for the
/// pets.
pub struct URow {
    pub group: &'static str,
    pub maps: Vec<usize>,
}

/// The list's rows in order, grouped like the game's facilities.
pub fn unlock_rows() -> Vec<URow> {
    let m = |id: &str| unlocks::find(id).into_iter().collect::<Vec<_>>();
    let mut v = vec![];
    for id in ["lab", "song", "supply", "coin"] {
        v.push(URow { group: "Facilities", maps: m(id) });
    }
    v.push(URow { group: "Pets", maps: vec![] });
    v.push(URow { group: "Pets", maps: m("costume") });
    for id in ["trader:items0", "trader:items1", "trader:words", "trader:scenes", "trader:costumes", "trader:delivery"] {
        v.push(URow { group: "Trader", maps: m(id) });
    }
    for id in ["gallery", "notes2", "tips", "combos", "events", "milestones"] {
        v.push(URow { group: "Records", maps: m(id) });
    }
    v.push(URow { group: "Shops", maps: m("shop:0") });
    v.push(URow { group: "Shops", maps: m("shop:1") });
    let armory: Vec<usize> = (0..unlocks::maps().len()).filter(|&i| unlocks::maps()[i].id.starts_with("armory:")).collect();
    v.push(URow { group: "Shops", maps: armory });
    v
}

/// The Armory's equipment type `t` (1-5 armor parts, 7-21 weapon classes).
fn equip_type(t: usize) -> &'static str {
    if t <= 5 { armor_parts().get(t.wrapping_sub(1)).copied().unwrap_or("?") } else { weapon_classes().get(t - 7).copied().unwrap_or("?") }
}

/// The name of map `m` in the UI.
pub fn unlock_map_name(m: usize) -> String {
    let id = unlocks::maps()[m].id.as_str();
    match id {
        "lab" => tr("Soaratorium Lab upgrades").into(),
        "song" => tr("Jukebox songs").into(),
        "supply" => tr("Supply drop sets").into(),
        "coin" => tr("Horns Coin trades").into(),
        "costume" => tr("Poogie and Moofy costumes").into(),
        "gallery" => tr("Housekeeper's Gallery").into(),
        "trader:items0" => tr("Trader · items").into(),
        "trader:items1" => tr("Trader · special items").into(),
        "trader:words" => tr("Trader · title words").into(),
        "trader:scenes" => tr("Trader · Guild Card scenes").into(),
        "trader:costumes" => tr("Trader · pet costumes").into(),
        "trader:delivery" => tr("Trader · delivery requests").into(),
        "notes2" => tr("Hunter's Notes · second list").into(),
        "tips" => tr("Hunter's Notes · tips read").into(),
        "combos" => tr("Combination List · recipes combined").into(),
        "events" => tr("One-time event scenes").into(),
        "milestones" => tr("Award milestones").into(),
        "shop:0" => tr("Market").into(),
        "shop:1" => tr("Guild Store").into(),
        _ => match id.strip_prefix("armory:").and_then(|t| t.parse().ok()) {
            Some(t) => trf("Armory · {}", &[&equip_type(t)]),
            None => id.into(),
        },
    }
}

fn row_name(r: &URow) -> String {
    match r.maps[..] {
        [] => tr("Village pets").into(),
        [m] => unlock_map_name(m),
        _ => tr("Armory").into(),
    }
}

/// What a list shows, in game terms.
fn row_note(r: &URow) -> String {
    let id = r.maps.first().map_or("", |&m| unlocks::maps()[m].id.as_str());
    match id {
        "" => tr("Names, costumes worn and adoption of Moofy and the three Poogies.").into(),
        "lab" => tr("Upgrades installed at the Soaratorium Lab. Installing one here also offers it, as the game installs only offered ones, and unlocks the supply drop set it gives. The three Item Box expansions stay once installed: a smaller box could lose what is stored past it.").into(),
        "song" => tr("Songs the Hunters' Pub Jukebox plays. The Mewstress unlocks the others when you talk to her.").into(),
        "supply" => tr("Sets the Provision Division can send. Most follow a Lab upgrade: the game gives them back each time the Division opens while the upgrade is installed.").into(),
        "coin" => tr("Items the Mewstress trades for Horns Coins.").into(),
        "costume" => tr("Costumes for the village pets: Moofy's (the first six) and the Poogies'. Ten, twenty, twenty-eight and every Poogie costume earn the Poogie Ball awards after your next quest.").into(),
        "gallery" => tr("Movies the Housekeeper's Gallery plays. Credits 2 and 3 come back at the next load once the ending is in the Gallery.").into(),
        "trader:words" | "trader:scenes" | "trader:costumes" => tr("What the Trader puts up for sale. An entry for sale can be bought even without the download that sells it; what you already have is not shown.").into(),
        "trader:items0" | "trader:items1" => tr("Items the Trader sells for Trader points.").into(),
        "trader:delivery" => tr("Delivery requests the Trader offers. The game offers one once its villager's request is raised.").into(),
        "notes2" => tr("Extra Hunter's Notes entries of the Elder Dragons and deviants. Unlocking one gives the monster's Notes page its NEW mark, as the game does.").into(),
        "tips" => tr("Hunter's Notes tips read. A tip not read shows NEW once the game offers it.").into(),
        "combos" => tr("Recipes combined at least once. A recipe never combined always succeeds the first time; 130 earn an award after your next quest.").into(),
        "events" => tr("Scenes that play once. Clearing one plays it again the next time its place loads.").into(),
        "milestones" => tr("What the award checks read. Each award follows after your next quest once all its milestones are set.").into(),
        _ => tr("Read only: the shop sells an entry once its star level allows, whatever the save holds; the save only keeps which entries got their NEW mark.").into(),
    }
}

/// A combination recipe: "Potion (Herb + Blue Mushroom)".
fn combo_name(b: usize) -> String {
    match tables().combinations.get(b) {
        Some(&(a, x, r, _)) => trf("{} ({} + {})", &[&assets::item_name(r), &assets::item_name(a), &assets::item_name(x)]),
        None => trf("Recipe {}", &[&b]),
    }
}

/// A name of the pack's list `v` at `i`, or "#i".
fn listed(v: &[String], i: usize) -> String {
    v.get(i).filter(|s| !s.is_empty()).cloned().unwrap_or_else(|| format!("#{i}"))
}

pub fn pet_costume_name(c: u8) -> String {
    listed(&assets::names().pet_costumes, c as usize)
}

/// The Housekeeper `i` (`unlocks::HOUSEKEEPER`), as Change Housekeeper names them.
pub fn housekeeper_name(i: u8) -> String {
    match i {
        0 => tr("Chamberlyne").into(),
        1 => tr("Guildmarm").into(),
        2 => tr("Moga Sweetheart").into(),
        3 => tr("Tanzia Sweetheart").into(),
        4 => tr("Headwhiskress").into(),
        5 => tr("Lil Miss Forge").into(),
        6 => tr("Funky Felyne").into(),
        _ => trf("Housekeeper {}", &[&i]),
    }
}

/// A place a load starts in (`unlocks::START_VILLAGE`), as the airship names it.
pub fn start_place_name(scene: u8) -> String {
    match scene {
        1 => tr("Bherna Village").into(),
        2 => tr("Kokoto Village").into(),
        3 => tr("Pokke Village").into(),
        4 => tr("Yukumo Village").into(),
        6 => tr("Soaratorium").into(),
        _ => trf("Scene {}", &[&scene]),
    }
}

/// Place `p` of the event scene table: 0 the Hub, 1-4 the villages, then the Palico
/// Ranch, the Wycademy and the Hunters' Pub.
fn place(p: usize) -> &'static str {
    match p {
        0 => tr("Hunters Hub"),
        1 => tr("Bherna"),
        2 => tr("Kokoto"),
        3 => tr("Pokke"),
        4 => tr("Yukumo"),
        5 => tr("Palico Ranch"),
        6 => tr("Wycademy"),
        7 => tr("Hunters' Pub"),
        _ => "?",
    }
}

fn group_name(g: &str) -> &'static str {
    match g {
        "Facilities" => tr("Facilities"),
        "Pets" => tr("Pets"),
        "Trader" => tr("Trader"),
        "Records" => tr("Records"),
        _ => tr("Shops"),
    }
}

/// One-time event scene `bit`: where it plays and when.
fn event_name(bit: usize, ev: u32) -> String {
    let at = place;
    match bit {
        0..=5 | 18 | 19 => trf("{}: first visit", &[&at(match bit {
            18 => 6,
            19 => 7,
            b => b,
        })]),
        6..=8 | 20 => trf("Palico Ranch: event {}", &[&ev]),
        9..=12 => trf("{}: Village ★4", &[&at(bit - 8)]),
        13..=16 => trf("{}: first urgent quests", &[&at(bit - 12)]),
        21..=23 => trf("Wycademy: event {}", &[&ev]),
        _ => trf("Event {}", &[&ev]),
    }
}

/// Milestone `bit` of the award checks' map, `id` as in data/unlock-lists.csv.
fn milestone_name(bit: usize, id: usize) -> String {
    let bias = |k: usize| tr(palico::BIASES.get(k).copied().unwrap_or("?"));
    match bit {
        0..=4 => trf("{} armor at its max level", &[&armor_parts().get(id.wrapping_sub(1)).copied().unwrap_or("?")]),
        5..=19 => trf("{} at its max level", &[&weapon_classes().get(id).copied().unwrap_or("?")]),
        20..=23 => trf("A Moofy or Poogie moment in {}", &[&place(id)]),
        24..=34 => trf("Footbath guest: {}", &[&tables().npc_name.get(&(id as u32)).cloned().unwrap_or_else(|| id.to_string())]),
        35..=41 => trf("A Palico of the {} forte hired", &[&bias(id)]),
        42..=56 => trf("{} past its second level threshold (read by nothing)", &[&weapon_classes().get(id).copied().unwrap_or("?")]),
        _ => trf("A Palico of the {} forte at level 99", &[&bias(id)]),
    }
}

/// The name of entry `e` of map `m`.
pub fn entry_name(m: usize, e: &UnlockEntry) -> String {
    let n = assets::names();
    let (b, id) = (e.bit, e.id as usize);
    let mp = unlocks::maps()[m].id.as_str();
    match mp {
        "lab" => listed(&n.lab, b),
        "song" => listed(&n.songs, b),
        "supply" => listed(&n.supply, b),
        "costume" => listed(&n.pet_costumes, b),
        "gallery" => listed(&n.gallery, b),
        "tips" => listed(&n.tips, b),
        "coin" | "trader:items0" | "trader:items1" | "shop:0" | "shop:1" => assets::item_name(id as u16),
        "trader:words" => listed(&n.gc_words, id),
        "trader:scenes" => listed(&n.gc_scenes, id),
        "trader:costumes" => listed(&n.pet_costumes, id),
        "trader:delivery" => match tables().requests.iter().find(|r| r.index == id) {
            Some(r) => trf("Delivery request {} · {}", &[&id, &tr(r.village.as_str())]),
            None => trf("Delivery request {}", &[&id]),
        },
        "notes2" => tables()
            .monster_meta
            .iter()
            .position(|x| x.notes_bit == Some(id))
            .and_then(|i| assets::monster_name(i + 1).map(String::from).or_else(|| tables().monsters.get(i).map(|x| x.name.clone())))
            .unwrap_or_else(|| format!("#{id}")),
        "combos" => combo_name(b),
        "events" => event_name(b, e.id),
        "milestones" => milestone_name(b, id),
        _ => match mp.strip_prefix("armory:").and_then(|t| t.parse::<usize>().ok()) {
            Some(t) if t <= 5 => n.armor.get(&t.to_string()).and_then(|v| v.iter().find(|p| p.id as usize == id)).map_or_else(|| format!("#{id}"), |p| p.name.clone()),
            Some(t) => n.weapons.get(&(t - 7).to_string()).and_then(|v| v.get(id)).map_or_else(|| format!("#{id}"), |p| p.name.clone()),
            None => format!("#{b}"),
        },
    }
}

/// What unlocks an entry in game, from its `need` (tools/unlock_lists.py).
fn need_text(need: &str) -> String {
    let one = |t: &str| -> String {
        let n = |p: &str| t.strip_prefix(p).and_then(|x| x.parse::<u32>().ok()).unwrap_or(0);
        match t {
            "start" => tr("from the start").into(),
            "hr" => tr("the HR limit released").into(),
            "dlc" => tr("a download").into(),
            "ex" => tr("a deviant's EX level cleared").into(),
            "open" => tr("opening the Provision Division").into(),
            "adopt" => tr("adopting the pet").into(),
            "talk" => tr("a villager's gift").into(),
            "trader" => tr("buying it at the Trader").into(),
            "visit" => tr("the first visit").into(),
            "view" => tr("reading it").into(),
            "combine" => tr("combining it").into(),
            "progress" => tr("story progress").into(),
            _ if t.starts_with("lab") => trf("the Lab upgrade {}", &[&listed(&assets::names().lab, n("lab") as usize)]),
            _ if t.starts_with("item") => trf("{} obtained", &[&assets::item_name(n("item") as u16)]),
            _ if t.starts_with('v') => trf("Village ★{}", &[&n("v")]),
            _ if t.starts_with('h') => trf("Hub ★{}", &[&n("h")]),
            _ if t.starts_with('q') => {
                let q = n("q");
                match tables().quests.iter().find(|x| x.id == q) {
                    Some(x) => trf("“{}” cleared", &[&x.name]),
                    None => trf("quest {} cleared", &[&q]),
                }
            }
            _ if t.starts_with('f') => trf("event flag {}", &[&n("f")]),
            _ => t.into(),
        }
    };
    need.split('|').map(|alt| alt.split('&').map(one).collect::<Vec<_>>().join(tr(" and "))).collect::<Vec<_>>().join(tr(" or "))
}

/// The second line of an entry: what unlocks it, or for a shop entry whether the shop
/// sells it now.
fn entry_sub(st: &State, m: usize, e: &UnlockEntry) -> String {
    let mp = &unlocks::maps()[m];
    let need = need_text(&e.need);
    if mp.kind == Kind::Marks {
        return match unlocks::need_holds(st.save(), st.slot, &e.need) {
            Some(true) => trf("On sale · {}", &[&need]),
            _ => trf("Not yet · {}", &[&need]),
        };
    }
    match mp.id.as_str() {
        "tips" | "milestones" | "events" | "combos" => String::new(),
        _ => match unlocks::refusal(st.save(), st.base(), m, e.bit, false) {
            Some(Refusal::BoxExpansion) if unlocks::on(st.save(), st.base(), m, e.bit) => trf("{} · stays installed", &[&need]),
            _ => need,
        },
    }
}

/// A shop entry is on sale now: its star condition holds.
fn on_sale(st: &State, e: &UnlockEntry) -> bool {
    unlocks::need_holds(st.save(), st.slot, &e.need) == Some(true)
}

/// (set, entries) of map `m` as its row counts them: the shops' entries on sale now.
fn shown_count(st: &State, m: usize) -> (usize, usize) {
    if unlocks::maps()[m].kind == Kind::Marks {
        let e = unlocks::entries(m);
        (e.iter().filter(|e| on_sale(st, e)).count(), e.len())
    } else {
        unlocks::count(st.save(), st.base(), m)
    }
}

fn sel_row(rows: &[URow]) -> usize {
    view(|v| v.unlock_sel).min(rows.len() - 1)
}

pub(super) fn unlocks_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let rows = unlock_rows();
    let sel = sel_row(&rows);
    let pets_changed = (0..unlocks::PETS).any(|k| !st.was(Target::Pet(k)).is_empty())
        || !st.was(Target::Moofahs).is_empty()
        || !st.was(Target::MoofahGifts).is_empty();
    let mut last = "";
    let list: Vec<UnlockRow> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let (k, n) = r.maps.iter().fold((0, 0), |a, &m| {
                let (k, n) = shown_count(st, m);
                (a.0 + k, a.1 + n)
            });
            let group = if r.group != last { group_name(r.group).to_string() } else { String::new() };
            last = r.group;
            UnlockRow {
                index: i as i32,
                group: group.into(),
                name: row_name(r).into(),
                count: if r.maps.is_empty() { String::new() } else { format!("{} / {}", num(k as i64), num(n as i64)) }.into(),
                changed: if r.maps.is_empty() { pets_changed } else { r.maps.iter().any(|&m| !st.was(Target::Unlock(m)).is_empty()) },
            }
        })
        .collect();
    api.set_unlock_rows(keep(api.get_unlock_rows(), list));
    api.set_unlock_sel(sel as i32);
    let r = &rows[sel];
    api.set_unlock_title(row_name(r).into());
    api.set_unlock_note(row_note(r).into());
    api.set_unlock_pets(r.maps.is_empty());
    api.set_unlock_editable(r.maps.iter().any(|&m| unlocks::maps()[m].kind != Kind::Marks));
    let editable: Vec<usize> = rows.iter().flat_map(|r| r.maps.iter().copied()).filter(|&m| unlocks::maps()[m].kind != Kind::Marks).collect();
    let (k, n) = editable.iter().fold((0, 0), |a, &m| {
        let (k, n) = unlocks::count(s, base, m);
        (a.0 + k, a.1 + n)
    });
    api.set_unlock_summary(trf("{} of {} entries unlocked", &[&num(k as i64), &num(n as i64)]).into());
    if r.maps.is_empty() {
        pets_view(ui, st);
        return;
    }
    let (q, missing) = view(|v| (v.unlock_search.to_lowercase(), v.unlock_missing));
    let orig = st.orig();
    let armory = r.maps.len() > 1;
    let cells: Vec<UnlockCell> = r
        .maps
        .iter()
        .flat_map(|&m| unlocks::entries(m).iter().map(move |e| (m, e)))
        .filter_map(|(m, e)| {
            let marks = unlocks::maps()[m].kind == Kind::Marks;
            let on = if marks { on_sale(st, e) } else { unlocks::on(s, base, m, e.bit) };
            let mut name = entry_name(m, e);
            if armory {
                let t: usize = unlocks::maps()[m].id["armory:".len()..].parse().unwrap_or(0);
                name = format!("{} · {name}", equip_type(t));
            }
            if (missing && on) || (!q.is_empty() && !name.to_lowercase().contains(&q)) {
                return None;
            }
            Some(UnlockCell {
                key: (m * 1024 + e.bit) as i32,
                name: name.into(),
                sub: entry_sub(st, m, e).into(),
                on,
                new: unlocks::is_new(s, base, m, e.bit),
                changed: unlocks::on(s, base, m, e.bit) != unlocks::on(orig, base, m, e.bit),
                enabled: unlocks::refusal(s, base, m, e.bit, !on).is_none(),
                check: !marks,
            })
        })
        .collect();
    api.set_unlock_cells(keep(api.get_unlock_cells(), cells));
}

fn pets_view(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let (s, base) = (st.save(), st.base());
    let places = [tr("Bherna"), tr("Kokoto"), tr("Pokke"), tr("Yukumo")];
    let pets: Vec<PetRow> = (0..unlocks::PETS)
        .map(|k| {
            let range = unlocks::costume_range(k);
            let c = unlocks::pet_costume(s, base, k);
            PetRow {
                index: k as i32,
                place: places[k].into(),
                name: unlocks::pet_name(s, base, k).into(),
                costume: if range.contains(&c) { (c - range.start()) as i32 } else { -1 },
                costumes: strings(range.map(pet_costume_name)),
                adopted: unlocks::adopted(s, base, k),
                was: st.was(Target::Pet(k)).into(),
            }
        })
        .collect();
    api.set_pets(model(pets));
    api.set_moofahs(model((0..unlocks::PETS).map(|k| unlocks::moofah(s, base, k) as i32).collect()));
    api.set_was_moofahs(st.was(Target::Moofahs).into());
    api.set_moofah_gifts(unlocks::moofah_gifts(s, base) as i32);
    api.set_was_moofah_gifts(st.was(Target::MoofahGifts).into());
}

/// Every Derived: from code, checked against the save timeline, not yet in game.
fn stage(ui: &AppWindow, s: &mut State, t: Target, f: impl FnOnce(&mut mhgu_save::Save, usize)) {
    if refused(ui, Conf::Derived) {
        return;
    }
    let title = t.label(s.save(), s.slot);
    s.edit(Edit::one(t, title, Conf::Derived), |sv, base| {
        f(sv, base);
        vec![]
    });
}

pub(super) fn wire_unlocks(ui: &AppWindow, st: &Shared) {
    on!(ui, st, on_select_unlock, |ui, s, i: i32| {
        view(|v| {
            v.unlock_sel = i.max(0) as usize;
            v.unlock_search.clear();
            v.unlock_missing = false;
        });
        let api = ui.global::<Api>();
        api.set_unlock_search("".into());
        api.set_unlock_missing(false);
        let _ = &s;
    });
    on!(ui, st, on_filter_unlocks, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.unlock_search = api.get_unlock_search().to_string();
            v.unlock_missing = api.get_unlock_missing();
        });
        let _ = &s;
    });
    on!(ui, st, on_set_unlock, |ui, s, key: i32, on: bool| {
        let (m, b) = (key as usize / 1024, key as usize % 1024);
        if m >= unlocks::maps().len() || unlocks::refusal(s.save(), s.base(), m, b, on).is_some() {
            return;
        }
        stage(&ui, &mut s, Target::Unlock(m), |sv, base| {
            unlocks::set(sv, base, m, b, on);
        });
    });
    on!(ui, st, on_set_pet, |ui, s, k: i32, field: SharedString, v: SharedString| {
        let k = (k.max(0) as usize).min(unlocks::PETS - 1);
        let slot = s.slot;
        stage(&ui, &mut s, Target::Pet(k), |sv, base| match field.as_str() {
            "name" => unlocks::set_pet_name(sv, base, k, v.trim()),
            "costume" => {
                let c = *unlocks::costume_range(k).start() as i32 + v.parse::<i32>().unwrap_or(0);
                unlocks::set_pet_costume(sv, base, k, c.clamp(0, 255) as u8);
            }
            _ => unlocks::set_adopted(sv, base, slot, k, v == "1"),
        });
    });
    on!(ui, st, on_set_moofah, |ui, s, k: i32, v: i32| {
        let k = (k.max(0) as usize).min(unlocks::PETS - 1);
        stage(&ui, &mut s, Target::Moofahs, |sv, base| unlocks::set_moofah(sv, base, k, v.clamp(0, 10) as u8));
    });
    on!(ui, st, on_set_moofah_gifts, |ui, s, v: i32| {
        let slot = s.slot;
        stage(&ui, &mut s, Target::MoofahGifts, |sv, _| unlocks::set_moofah_gifts(sv, slot, v.clamp(0, 10) as u8));
    });
}
