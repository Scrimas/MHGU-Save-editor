//! Quick goals and page-level bulk actions: edits that write what playing would have
//! written, planned on a copy of the save and described in game terms (03, R3, R4).

use crate::assets;
use crate::fmt::{list, num};
use crate::i18n::{tr, trf, trn};
use crate::state::Conf;
use crate::targets::{Mon, Target};
use mhgu_save::character as ch;
use mhgu_save::data::tables;
use mhgu_save::items::{self, Stack, Store};
use mhgu_save::progress::{Char, QuestBit, DEVIANTS, VILLAGES};
use mhgu_save::save::Save;
use mhgu_save::guildcard as gc;
use mhgu_save::{monsters, smithy, unlocks};

pub struct Def {
    pub id: &'static str,
    pub conf: Conf,
}

/// The Overview's goals (titles and copy come from `plan`). All confirmed: tested in game
/// (arts and canteen as written, quests by the bulk completion of 2026-09-19, crowns by
/// the game's award check after the 2026-10-04 write, notes and HR by the controlled
/// write of 2026-10-05; items obtained, the Smithy lists and the card's titles, scenes and
/// poses in game on 2026-10-09) or every field written is CONFIRMED by the save timeline
/// (awards, money; tools/evidence).
///
/// The unlock goals after them follow their maps (`views::maps_conf`, checked in game on
/// 2026-10-10): the Trader's is Derived, from code and the save timeline
/// (`mhgu_save::unlocks`).
pub const GOALS: [Def; 17] = [
    Def { id: "quests", conf: Conf::Confirmed },
    Def { id: "arts", conf: Conf::Confirmed },
    Def { id: "canteen", conf: Conf::Confirmed },
    Def { id: "awards", conf: Conf::Confirmed },
    Def { id: "notes", conf: Conf::Confirmed },
    Def { id: "crowns", conf: Conf::Confirmed },
    Def { id: "hr999", conf: Conf::Confirmed },
    Def { id: "money", conf: Conf::Confirmed },
    Def { id: "obtained", conf: Conf::Confirmed },
    Def { id: "smithy", conf: Conf::Confirmed },
    Def { id: "card", conf: Conf::Confirmed },
    Def { id: "lab", conf: Conf::Confirmed },
    Def { id: "costumes", conf: Conf::Confirmed },
    Def { id: "songs", conf: Conf::Confirmed },
    Def { id: "trader", conf: Conf::Derived },
    Def { id: "combos", conf: Conf::Confirmed },
    Def { id: "gallery", conf: Conf::Confirmed },
];

/// The unlock maps (`unlocks::maps()` ids) a goal unlocks every entry of.
fn goal_maps(id: &str) -> &'static [&'static str] {
    match id {
        "lab" => &["lab"],
        "costumes" => &["costume"],
        "songs" => &["song"],
        "trader" => &["trader:words", "trader:scenes", "trader:costumes"],
        "combos" => &["combos"],
        "gallery" => &["gallery"],
        _ => &[],
    }
}

/// The maps of a goal or of the Unlocks page's row `row` ("unlocks:all:<row>").
fn unlock_maps(parts: &[&str]) -> Vec<usize> {
    match parts {
        ["unlocks", _, row] => row.parse::<usize>().ok().and_then(|r| crate::views::unlock_rows().into_iter().nth(r)).map(|r| r.maps).unwrap_or_default(),
        [g] => goal_maps(g).iter().filter_map(|id| unlocks::find(id)).collect(),
        _ => vec![],
    }
}

/// Entries of `maps` that differ between two saves.
fn unlock_changes(a: &Save, b: &Save, base: usize, maps: &[usize]) -> usize {
    maps.iter().map(|&m| unlocks::entries(m).iter().filter(|e| unlocks::on(a, base, m, e.bit) != unlocks::on(b, base, m, e.bit)).count()).sum()
}

/// The names of a Guild Card unlock map, from the asset pack.
fn card_names(m: gc::Map) -> &'static [String] {
    let n = assets::names();
    match m {
        gc::Map::Words => &n.gc_words,
        gc::Map::Links => &n.gc_links,
        gc::Map::Scenes => &n.gc_scenes,
        gc::Map::Poses => &n.gc_poses,
    }
}

/// Item IDs the game can mark obtained and the asset pack names (none without it).
fn real_items() -> impl Iterator<Item = u16> {
    let n = &assets::names().items;
    (1..=items::OBTAINED_MAX_ID).filter(|&id| n.get(id as usize).is_some_and(|x| !x.is_empty() && x != "DUMMY" && x != "(None)" && !x.starts_with('-')))
}

/// What a goal or bulk action would do to this save.
#[derive(Default)]
pub struct Plan {
    pub title: String,
    /// Card and preview copy, with counts from the save.
    pub summary: String,
    /// "120 records"
    pub count: String,
    pub conf: Option<Conf>,
    pub targets: Vec<Target>,
    /// (name, before, after) of every value that changes.
    pub lines: Vec<(String, String, String)>,
    /// What stays as it is, or follows later in game.
    pub note: String,
    pub bytes: usize,
    pub tech: String,
    /// Button that adds it to Review.
    pub action: String,
    /// The Review entry: title and second line.
    pub review: String,
    pub detail: String,
    /// Can't run on this character yet (HR 999 before the Hub is joined).
    pub blocked: bool,
}

/// Store of an "items:…:box|pouch" id.
fn store(id: &str) -> Store {
    if id.ends_with(":pouch") { Store::Pouch } else { Store::Box }
}

/// Run goal or bulk action `id` on `s`; returns the values it changes and a note.
pub fn apply(id: &str, s: &mut Save, slot: usize) -> Vec<Target> {
    let base = s.base(slot);
    let mut out = vec![];
    let parts: Vec<&str> = id.split(':').collect();
    match parts[..] {
        ["quests"] => {
            let mut c = Char::new(s, slot);
            let todo: Vec<usize> = Char::real_quests(true).iter().filter(|q| !c.quest(QuestBit::Cleared, q.index)).map(|q| q.index).collect();
            c.clear_quests(&todo);
            out.extend(todo.iter().map(|&i| Target::Quest(i)));
            let mut seen = std::collections::HashSet::new();
            for o in &tables().offers {
                if o.quest_id.is_none() || !seen.insert(o.index) || c.flag(o.accept_flag) {
                    continue;
                }
                c.accept_request(o.index);
                out.push(Target::Request(o.index));
            }
        }
        ["arts"] | ["checks", "all", "0"] | ["checks", "none", "0"] => {
            let on = !id.contains("none");
            let mut c = Char::new(s, slot);
            let ids: Vec<u32> = tables().arts.iter().map(|a| a.0).filter(|&i| c.art(i) != on).collect();
            for i in ids {
                c.set_art(i, on);
                out.push(Target::Art(i));
            }
        }
        ["canteen"] | ["checks", _, "1"] | ["checks", _, "2"] => {
            let on = !id.contains("none");
            let mut c = Char::new(s, slot);
            if !id.ends_with(":1") {
                let ids: Vec<usize> = (0..45).filter(|&i| c.ingredient(i) != on).collect();
                for i in ids {
                    c.set_ingredient(i, on);
                    out.push(Target::Ingredient(i));
                }
            }
            if !id.ends_with(":2") {
                let ids: Vec<usize> = (0..99).filter(|&i| c.dish(i) != on).collect();
                for i in ids {
                    c.set_dish(i, on);
                    out.push(Target::Dish(i));
                }
            }
        }
        // bits 100 (Veteran Hunter's Prize, from a save transfer) and 101 share one
        // slot of the card and stay; both award maps are written (docs/09)
        ["awards"] | ["checks", "all", "3"] => {
            let mut c = Char::new(s, slot);
            for b in (0..100).chain(102..132) {
                if !c.award(b) {
                    out.push(Target::Award(b));
                }
                c.set_award(b, true);
            }
        }
        ["checks", "none", "3"] => {
            let mut c = Char::new(s, slot);
            for b in 0..132 {
                if c.award(b) {
                    out.push(Target::Award(b));
                }
                c.set_award(b, false);
            }
        }
        ["notes"] => {
            for i in 1..=monsters::N {
                if monsters::notes(s, base, i) == Some(false) {
                    monsters::set_notes(s, base, i, true);
                    out.push(Target::Monster(i, Mon::Notes));
                }
            }
        }
        ["crowns"] => {
            for i in 1..=monsters::N {
                let Some((lo, hi)) = monsters::crown_sizes(i) else { continue };
                let mut r = monsters::get(s, base, i);
                // only sizes its quests give: the crown sizes, or the nearest a quest gives when
                // none reaches them; a record already past them stays, one outside the
                // quests' range comes back into it
                let (a, b) = if monsters::meta(i).fixed_size { (100, 100) } else { monsters::quest_range(i).unwrap_or((1, u16::MAX)) };
                let ok = |v: u16| v != 0 && (a..=b).contains(&v);
                let (lo, hi) = (lo.clamp(a, b), hi.clamp(a, b));
                let (nlo, nhi) = (if ok(r.min) { r.min.min(lo) } else { lo }, if ok(r.max) { r.max.max(hi) } else { hi });
                if nlo != r.min {
                    out.push(Target::Monster(i, Mon::Min));
                }
                if nhi != r.max {
                    out.push(Target::Monster(i, Mon::Max));
                }
                if (nlo, nhi) != (r.min, r.max) {
                    r.min = nlo;
                    r.max = nhi;
                    monsters::set(s, base, i, r);
                }
            }
        }
        ["monsters", "hunts"] => {
            for i in 1..=monsters::N {
                let m = &tables().monsters[i - 1];
                if m.name.is_empty() || (106..=112).contains(&i) {
                    continue;
                }
                let mut r = monsters::get(s, base, i);
                if r.hunts < monsters::MAX_COUNT {
                    r.hunts = monsters::MAX_COUNT;
                    monsters::set(s, base, i, r);
                    out.push(Target::Monster(i, Mon::Hunts));
                }
            }
        }
        ["hr999"] => {
            if ch::get(s, base).hr != 999 && ch::set_hr(s, base, 999) {
                out.extend([Target::Hr, Target::HrPoints]);
            }
        }
        ["money"] => {
            ch::set_funds(s, base, ch::MAX_FUNDS);
            ch::set_wycademy(s, base, ch::MAX_POINTS);
            out.extend([Target::Funds, Target::Wycademy]);
            for v in 0..4 {
                for g in [false, true] {
                    ch::set_village_points(s, base, v, g, ch::MAX_VILLAGE_POINTS);
                    out.push(Target::Points(v, g));
                }
            }
        }
        ["obtained"] => {
            let todo: Vec<u16> = real_items().filter(|&id| !items::obtained(s, base, id)).collect();
            for &id in &todo {
                items::set_obtained(s, base, id, true);
            }
            if !todo.is_empty() {
                out.push(Target::Obtained);
            }
        }
        ["smithy"] => {
            let gender = ch::get(s, base).gender;
            for (i, l) in smithy::lists().iter().enumerate() {
                if smithy::list_all(s, base, l, gender) > 0 {
                    out.push(Target::Smithy(i));
                }
            }
        }
        // only entries the pack names, never a reserve cell (nothing without the pack)
        ["card"] => {
            for (k, &m) in gc::MAPS.iter().enumerate() {
                let names = card_names(m);
                let (_, _, n) = m.at();
                let todo: Vec<usize> = (0..n).filter(|&i| !names.is_empty() && crate::warnings::card_entry(names, m, i) && !gc::unlocked(s, base, m, i)).collect();
                for &i in &todo {
                    gc::set_unlocked(s, base, m, i, true);
                }
                if !todo.is_empty() {
                    out.push(Target::CardMap(k));
                }
            }
        }
        ["lab"] | ["costumes"] | ["songs"] | ["trader"] | ["combos"] | ["gallery"] | ["unlocks", _, _] => {
            let on = parts.get(1) != Some(&"none");
            for m in unlock_maps(&parts) {
                if unlocks::set_all(s, base, m, on) > 0 {
                    out.push(Target::Unlock(m));
                }
            }
        }
        ["deviants", "permits"] => {
            let mut c = Char::new(s, slot);
            for d in 0..DEVIANTS.len() {
                if c.permits(d) < 99 {
                    c.set_permits(d, 99);
                    out.push(Target::Permits(d));
                }
            }
        }
        // every level cleared and seen, as one deviant's "Levels cleared" does
        ["deviants", "levels"] => {
            let mut c = Char::new(s, slot);
            for d in 0..DEVIANTS.len() {
                let (q0, n) = Char::deviant_levels(d);
                if (0..n).all(|k| c.quest(QuestBit::Cleared, q0 + k)) {
                    continue;
                }
                for k in 0..n {
                    c.set_quest(QuestBit::Cleared, q0 + k, true);
                    c.set_quest(QuestBit::Seen, q0 + k, true);
                }
                out.push(Target::Levels(d));
            }
        }
        ["items", what, _] => {
            let st = store(id);
            let v = items::all(s, base, st);
            let n: Vec<Stack> = match what {
                "sort" => items::compact(&v, true, st, |id| assets::item_max(id, st)),
                "max" => v.iter().map(|x| if x.is_empty() { *x } else { Stack { id: x.id, count: assets::item_max(x.id, st) } }).collect(),
                _ => vec![Stack::default(); v.len()],
            };
            items::set_all(s, base, st, &n);
            out.extend((0..v.len()).filter(|&i| v[i] != n.get(i).copied().unwrap_or_default()).map(|i| Target::Item(st, i)));
        }
        _ => {}
    }
    out
}

/// What `id` would change in this save, in game terms.
pub fn plan(id: &str, s: &Save, slot: usize) -> Plan {
    let mut c = s.clone();
    let targets = apply(id, &mut c, slot);
    let mut p = Plan { action: tr("Add to Review").into(), detail: tr("Quick goal").into(), ..Default::default() };
    for t in &targets {
        let (a, b) = (t.read(s, slot), t.read(&c, slot));
        if a != b {
            p.lines.push((t.label(&c, slot), a, b));
        }
    }
    // only values that show a change count; the rest are copies written alongside
    p.targets = targets.into_iter().filter(|t| t.read(s, slot) != t.read(&c, slot)).collect();
    let changed: Vec<usize> = c.bytes().iter().zip(s.bytes()).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
    p.bytes = changed.len();
    p.tech = tech(&changed, s.base(slot));
    let n = p.targets.len();
    let names = |f: &dyn Fn(&Target) -> bool| -> Vec<String> { p.targets.iter().filter(|t| f(t)).map(|t| t.label(&c, slot)).collect() };
    let base = s.base(slot);
    match id.split(':').collect::<Vec<_>>()[..] {
        ["quests"] => {
            let q = p.targets.iter().filter(|t| matches!(t, Target::Quest(_))).count();
            let r = n - q;
            let mut by_cat = std::collections::BTreeMap::<String, usize>::new();
            for t in &p.targets {
                if let Target::Quest(i) = t
                    && let Some(x) = tables().quests.iter().find(|x| x.index == *i)
                {
                    *by_cat.entry(x.category.clone()).or_default() += 1;
                }
            }
            let cats = by_cat.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
            p.title = tr("Complete every quest").into();
            let quests = trn("{} quest", "{} quests", q as i64, &[&num(q as i64)]);
            let requests = trn("{} villager request", "{} villager requests", r as i64, &[&num(r as i64)]);
            p.summary = if q == 0 && r == 0 {
                tr("Every quest is cleared.").into()
            } else if q == 0 {
                trf("Every quest is cleared. Posts the {} their villagers would offer.", &[&requests])
            } else if r > 0 {
                trf("Clears the {} not cleared yet ({}) and posts the {} their villagers would offer.", &[&quests, &cats, &requests])
            } else {
                trf("Clears the {} not cleared yet ({}).", &[&quests, &cats])
            };
            let k = q.max(r);
            p.count = if q > 0 { trn("{} quest", "{} quests", k as i64, &[&num(k as i64)]) } else { trn("{} request", "{} requests", k as i64, &[&num(k as i64)]) };
            p.note = tr("Star levels and story events stay to the game: they follow after your next quest. Completed quest sets are recorded too.").into();
        }
        ["arts"] => {
            p.title = tr("Unlock all Hunter Arts").into();
            p.summary = if n == 0 {
                tr("Every Hunter Art is unlocked.").into()
            } else {
                trf("Unlocks the {} you're missing: {}.", &[&trn("{} Hunter Art", "{} Hunter Arts", n as i64, &[&num(n as i64)]), &list(&names(&|_| true), 3)])
            };
            p.count = trn("{} art", "{} arts", n as i64, &[&num(n as i64)]);
        }
        ["canteen"] => {
            let d = p.targets.iter().filter(|t| matches!(t, Target::Dish(_))).count();
            let i = n - d;
            let dishes = trn("{} dish", "{} dishes", d as i64, &[&num(d as i64)]);
            let ingredients = trn("{} ingredient", "{} ingredients", i as i64, &[&num(i as i64)]);
            p.title = tr("Canteen dishes and ingredients").into();
            p.summary = match (d, i) {
                (0, 0) => tr("Every Canteen dish and ingredient is unlocked.").into(),
                (_, 0) => trf("Unlocks the {} you're missing in the Canteen.", &[&dishes]),
                (0, _) => trf("Unlocks the {} you're missing in the Canteen.", &[&ingredients]),
                _ => trf("Unlocks the {} and {} you're missing in the Canteen.", &[&dishes, &ingredients]),
            };
            p.count = trn("{} entry", "{} entries", n as i64, &[&num(n as i64)]);
        }
        ["awards"] => {
            p.title = tr("All Guild Card awards").into();
            let awards = trn("{} award", "{} awards", n as i64, &[&num(n as i64)]);
            p.summary = if n == 0 {
                tr("Your Guild Card has every award an edit can give.").into()
            } else {
                trf("Unlocks the {} you're missing: {}.", &[&awards, &list(&names(&|_| true), 3)])
            };
            p.count = awards;
            p.note = tr("Veteran Hunter's Prize comes only from a save transfer and stays as it is. Both of the game's award lists are written, as the game does.").into();
        }
        ["notes"] => {
            p.title = tr("Hunter's Notes").into();
            let mons: Vec<String> = p.targets.iter().filter_map(|t| if let Target::Monster(i, _) = t { Some(tables().monsters[i - 1].name.clone()) } else { None }).collect();
            p.summary = if n == 0 {
                tr("Every Hunter's Notes page is unlocked.").into()
            } else {
                trf("Unlocks the {} you're missing: {}.", &[&trn("{} Hunter's Notes page", "{} Hunter's Notes pages", n as i64, &[&num(n as i64)]), &list(&mons, 3)])
            };
            p.count = trn("{} page", "{} pages", n as i64, &[&num(n as i64)]);
            p.note = tr("Each page gets its NEW mark, as the game's own unlock does.").into();
        }
        ["crowns"] => {
            let gold = p.targets.iter().filter(|t| matches!(t, Target::Monster(_, Mon::Max))).count();
            let mini = p.targets.iter().filter(|t| matches!(t, Target::Monster(_, Mon::Min))).count();
            let mons = |k: usize| trn("{} monster", "{} monsters", k as i64, &[&num(k as i64)]);
            p.title = tr("Every crown").into();
            p.summary = match (gold, mini) {
                (0, 0) => tr("Every large monster has its mini and gold crown.").into(),
                (_, 0) => trf("Gold crown records for the {} without one. Also rebuilds the Guild Card monster log.", &[&mons(gold)]),
                (0, _) => trf("Mini crown records for the {} without one. Also rebuilds the Guild Card monster log.", &[&mons(mini)]),
                _ => trf(
                    "Gold crown records for the {} without one, and mini crown records for the {} without one. Also rebuilds the Guild Card monster log.",
                    &[&mons(gold), &mons(mini)],
                ),
            };
            p.count = trn("{} record", "{} records", n as i64, &[&num(n as i64)]);
            p.note = tr("The crown awards follow after your next quest in game.").into();
            p.detail = tr("Quick goal · also rebuilds the Guild Card monster log").into();
        }
        ["hr999"] => {
            p.title = tr("HR 999").into();
            let st = ch::get(s, base);
            if s.u16(base + ch::HUB_STAR) == 0 {
                p.blocked = true;
                p.summary = tr("Join the Hub in game first: until then HR follows the Hub's star level.").into();
                p.count = tr("Not available yet").into();
            } else if n == 0 {
                p.summary = tr("HR is already 999.").into();
            } else {
                p.summary = trf("Raises HR {} to 999 and releases the HR limit, as the game does at 4,246,430 HR points.", &[&num(st.hr)]);
                p.count = tr("HR 999").into();
            }
        }
        ["money"] => {
            p.title = tr("Max zenny and points").into();
            let has = |t: Target| p.targets.contains(&t);
            let vills = |g: bool| -> Vec<String> { (0..4).filter(|&v| has(Target::Points(v, g))).map(|v| tr(VILLAGES[v]).to_string()).collect() };
            let (lr, g) = (vills(false), vills(true));
            let mut doing = vec![];
            let mut done = vec![];
            if has(Target::Funds) { doing.push(tr("zenny to 9,999,999").to_string()) } else { done.push(tr("zenny").to_string()) }
            if has(Target::Wycademy) { doing.push(tr("Wycademy points to 9,999,999").to_string()) } else { done.push(tr("Wycademy points").to_string()) }
            match g.len() {
                0 => done.push(tr("G rank points").into()),
                4 => doing.push(tr("G rank village points to 20,000 in all four villages").into()),
                _ => doing.push(trf("G rank points to 20,000 in {}", &[&list(&g, 4)])),
            }
            match lr.len() {
                0 => done.push(tr("Low rank points").into()),
                4 => doing.push(tr("Low rank village points to 20,000 in all four villages").into()),
                _ => doing.push(trf("Low rank points to 20,000 in {}", &[&list(&lr, 4)])),
            }
            let first = cap(&list(&doing, 4));
            let done_txt = match done.len() {
                0 => String::new(),
                1 => trf("{} is already at the max", &[&cap(&done[0])]),
                _ => trf("{} are already at the max", &[&cap(&list(&done, 4))]),
            };
            p.summary = if doing.is_empty() {
                tr("Zenny and every point total are at the max.").into()
            } else if done.is_empty() {
                trf("{}.", &[&first])
            } else {
                trf("{}. {}.", &[&first, &done_txt])
            };
            if !done_txt.is_empty() && !doing.is_empty() {
                p.note = tr("Totals already at the max stay as they are.").into();
            }
            p.count = trn("{} value", "{} values", n as i64, &[&num(n as i64)]);
        }
        ["obtained"] => {
            p.title = tr("Every item obtained").into();
            let k = real_items().filter(|&id| !items::obtained(s, base, id)).count() as i64;
            let its = trn("{} item", "{} items", k, &[&num(k)]);
            if !assets::available() {
                p.blocked = true;
                p.summary = tr("Needs the game's item list, which this build does not have.").into();
                p.count = tr("Not available").into();
            } else if k == 0 {
                p.summary = tr("Every item of the game is marked obtained.").into();
            } else {
                p.summary = trf("Marks the {} you never had as obtained, as picking one up does. The Smithy then shows every material's name and the equipment those materials make.", &[&its]);
                p.count = its;
            }
            p.note = tr("A Smithy entry also needs the game's progress to allow it. Items obtained are read by the shop, the Trader and some villagers' requests too.").into();
        }
        ["smithy"] => {
            p.title = tr("List every Smithy entry").into();
            let gender = ch::get(s, base).gender;
            let k: usize = smithy::lists().iter().map(|l| {
                let (a, b) = smithy::count(s, base, l, gender);
                b - a
            }).sum();
            let entries = trn("{} entry", "{} entries", k as i64, &[&num(k as i64)]);
            p.summary = if n == 0 {
                tr("The Smithy lists every weapon, armor piece, decoration and Palico item.").into()
            } else {
                trf("Lists the {} the Smithy does not show yet, in {}, each with its NEW mark, as the game does when it first lists one.", &[&entries, &trn("{} list", "{} lists", n as i64, &[&num(n as i64)])])
            };
            p.count = entries;
            p.note = tr("Making them still takes their materials. Armor for the other body type stays unlisted, as in game.").into();
        }
        ["card"] => {
            p.title = tr("Every Guild Card title word, scene and pose").into();
            let k: usize = gc::MAPS
                .iter()
                .map(|&m| {
                    let (names, (_, _, n)) = (card_names(m), m.at());
                    (0..n).filter(|&i| !names.is_empty() && crate::warnings::card_entry(names, m, i) && !gc::unlocked(s, base, m, i)).count()
                })
                .sum();
            let entries = trn("{} entry", "{} entries", k as i64, &[&num(k as i64)]);
            if assets::names().gc_words.is_empty() {
                p.blocked = true;
                p.summary = tr("Needs the game's Guild Card texts, which this build does not have.").into();
                p.count = tr("Not available").into();
            } else if k == 0 {
                p.summary = tr("Every title word, scene and pose is unlocked.").into();
            } else {
                p.summary = trf("Unlocks the {} still locked for the Guild Card editor: title words, linking words, scenes and poses, each with its NEW mark.", &[&entries]);
                p.count = entries;
            }
            p.note = tr("Some come from downloads, the Trader or collaborations in game.").into();
        }
        ["monsters", "hunts"] => {
            let mons = trn("{} monster", "{} monsters", n as i64, &[&num(n as i64)]);
            p.title = tr("Max hunts").into();
            p.summary = if n == 0 { tr("Every monster is hunted 9,999 times.").into() } else { trf("Sets Hunted to 9,999 for the {} below it. Also rebuilds the Guild Card monster log.", &[&mons]) };
            p.count = mons;
            p.review = tr("Every monster hunted 9,999 times").into();
            p.detail = tr("Monsters · also rebuilds the Guild Card monster log").into();
        }
        ["items", what, _] => {
            let st = store(id);
            let pouch = st == Store::Pouch;
            let stacks = items::all(s, base, st).iter().filter(|x| !x.is_empty()).count();
            let slots = trn("{} slot", "{} slots", n as i64, &[&num(n as i64)]);
            match what {
                "sort" => {
                    p.title = if pouch { tr("Sort the pouch") } else { tr("Sort the item box") }.into();
                    p.summary = match (n, pouch) {
                        (0, true) => tr("The pouch is already sorted.").into(),
                        (0, false) => tr("The item box is already sorted.").into(),
                        (_, true) => trf("Sorts the pouch by item and merges stacks of the same item into one, up to its carry limit (the rest is dropped); {} change.", &[&slots]),
                        (_, false) => trf("Sorts the item box by item and merges stacks of the same item; {} change.", &[&slots]),
                    };
                    p.review = if pouch { tr("Pouch: sorted and merged") } else { tr("Item box: sorted and merged") }.into();
                }
                "max" => {
                    let k = trn("{} stack", "{} stacks", n as i64, &[&num(n as i64)]);
                    p.title = tr("Max counts").into();
                    if pouch {
                        p.summary = if n == 0 { tr("Every stack in the pouch is at its carry limit.").into() } else { trf("Sets the {} in the pouch to the most the game lets you carry (Potion 10, Max Potion 2, Ancient Potion 1…).", &[&k]) };
                        p.review = tr("Pouch: every stack at its carry limit").into();
                    } else {
                        p.summary = if n == 0 { tr("Every stack in the item box is at 99.").into() } else { trf("Raises the {} below 99 in the item box to 99.", &[&k]) };
                        p.review = tr("Item box: every stack ×99").into();
                    }
                }
                _ => {
                    let k = trn("{} stack", "{} stacks", stacks as i64, &[&num(stacks as i64)]);
                    p.title = if pouch { tr("Empty the pouch") } else { tr("Empty the item box") }.into();
                    p.summary = match (stacks, pouch) {
                        (0, true) => tr("The pouch is already empty.").into(),
                        (0, false) => tr("The item box is already empty.").into(),
                        (_, true) => trf("Removes all {} from the pouch. You can undo it in Review until you write.", &[&k]),
                        (_, false) => trf("Removes all {} from the item box. You can undo it in Review until you write.", &[&k]),
                    };
                    p.review = if pouch { trf("Pouch emptied: {}", &[&k]) } else { trf("Item box emptied: {}", &[&k]) };
                    p.action = if pouch { tr("Empty pouch") } else { tr("Empty box") }.into();
                }
            }
            p.count = slots;
            p.detail = if pouch { tr("Pouch") } else { tr("Item box") }.into();
        }
        [g @ ("lab" | "costumes" | "songs" | "trader" | "combos" | "gallery")] => {
            let k = unlock_changes(s, &c, base, &unlock_maps(&[g]));
            let ki = k as i64;
            let entries = trn("{} entry", "{} entries", ki, &[&num(ki)]);
            let (title, done, todo, note) = match g {
                "lab" => (
                    tr("Every Soaratorium Lab upgrade"),
                    tr("Every Lab upgrade is installed."),
                    trf("Installs the {} not installed yet, without spending Wycademy points or materials: the Item Box grows to its largest size and the Provision Division gets every supply drop set.", &[&trn("{} Lab upgrade", "{} Lab upgrades", ki, &[&num(ki)])]),
                    tr("Each upgrade is offered too, as the game installs only offered ones. Box expansions can't be removed again here."),
                ),
                "costumes" => (
                    tr("Every Poogie and Moofy costume"),
                    tr("Your pets have every costume."),
                    trf("Unlocks the {} your pets don't have yet, each with its NEW mark.", &[&trn("{} costume", "{} costumes", ki, &[&num(ki)])]),
                    tr("The Poogie Ball and Moofy Ball awards follow after your next quest. Some costumes come from downloads in game."),
                ),
                "songs" => (
                    tr("Every Jukebox song"),
                    tr("The Jukebox has every song."),
                    trf("Unlocks the {} the Hunters' Pub Jukebox can't play yet.", &[&trn("{} song", "{} songs", ki, &[&num(ki)])]),
                    "",
                ),
                "trader" => (
                    tr("Everything the Trader sells"),
                    tr("The Trader offers everything it can sell."),
                    trf("Puts up for sale the {} the Trader does not offer yet: title words, Guild Card scenes and pet costumes, the downloads' ones included.", &[&entries]),
                    tr("Buying still takes Trader points; what you already have is not shown."),
                ),
                "combos" => (
                    tr("Every combination recipe combined"),
                    tr("Every combination recipe has been combined."),
                    trf("Marks the {} never combined as combined. The award for 130 recipes follows after your next quest.", &[&trn("{} recipe", "{} recipes", ki, &[&num(ki)])]),
                    tr("A recipe never combined always succeeds the first time; these lose that bonus."),
                ),
                _ => (
                    tr("Every Gallery movie"),
                    tr("The Housekeeper's Gallery has every movie."),
                    trf("Adds the {} missing to the Housekeeper's Gallery, each with its NEW mark.", &[&trn("{} movie", "{} movies", ki, &[&num(ki)])]),
                    "",
                ),
            };
            p.title = title.into();
            p.summary = if k == 0 { done.into() } else { todo };
            p.count = entries;
            p.note = note.into();
        }
        ["unlocks", on, row] => {
            let maps = unlock_maps(&["unlocks", on, row]);
            let k = unlock_changes(s, &c, base, &maps) as i64;
            let entries = trn("{} entry", "{} entries", k, &[&num(k)]);
            let what = crate::views::unlock_rows().into_iter().nth(row.parse().unwrap_or(usize::MAX)).map_or(String::new(), |r| match r.maps[..] {
                [m] => crate::views::unlock_map_name(m),
                _ => String::new(),
            });
            if on == "all" {
                p.title = trf("Unlock all · {}", &[&what]);
                p.summary = if k == 0 { tr("Every entry is unlocked.").into() } else { trf("Unlocks the {} not unlocked yet, each with its NEW mark where the game keeps one.", &[&entries]) };
            } else {
                p.title = trf("Lock all · {}", &[&what]);
                p.summary = if k == 0 { tr("No entry is unlocked.").into() } else { trf("Locks the {} unlocked now.", &[&entries]) };
                if maps.iter().any(|&m| matches!(unlocks::maps()[m].id.as_str(), "lab" | "supply")) {
                    p.note = tr("Item Box expansions and the supply drop sets of installed Lab upgrades stay.").into();
                }
            }
            p.count = entries;
            p.conf = Some(crate::views::maps_conf(&maps));
            p.detail = tr("Unlocks").into();
        }
        ["deviants", "permits"] => {
            let devs = trn("{} deviant", "{} deviants", n as i64, &[&num(n as i64)]);
            p.title = tr("Max Special Permits").into();
            p.summary = if n == 0 { tr("Every deviant has 99 Special Permits.").into() } else { trf("Sets Special Permits to 99 for the {} below it.", &[&devs]) };
            p.count = devs;
            p.review = tr("Every deviant: 99 Special Permits").into();
            p.detail = tr("Collections").into();
        }
        ["deviants", "levels"] => {
            let devs = trn("{} deviant", "{} deviants", n as i64, &[&num(n as i64)]);
            p.title = tr("Every deviant level").into();
            p.summary = if n == 0 { tr("Every deviant has every level cleared.").into() } else { trf("Marks every level of the {} cleared: {}.", &[&devs, &list(&names(&|_| true), 3)]) };
            p.count = devs;
            p.note = tr("G-rank levels appear in game only once a G-rank quest against the base monster is cleared; each row says which.").into();
            p.review = tr("Every deviant level cleared").into();
            p.detail = tr("Collections").into();
        }
        ["checks", on, tab] => {
            let all = on == "all";
            let (what, check, uncheck) = match tab {
                "0" => (tr("Hunter Arts"), tr("Check every Hunter Art"), tr("Uncheck every Hunter Art")),
                "1" => (tr("Canteen dishes"), tr("Check every Canteen dish"), tr("Uncheck every Canteen dish")),
                "2" => (tr("Canteen ingredients"), tr("Check every Canteen ingredient"), tr("Uncheck every Canteen ingredient")),
                _ => (tr("awards"), tr("Check every award"), tr("Uncheck every award")),
            };
            let entries = trn("{} entry", "{} entries", n as i64, &[&num(n as i64)]);
            p.title = if all { check } else { uncheck }.into();
            p.summary = match (all, n) {
                (true, 0) => trf("Every one of the {} is already checked.", &[&what]),
                (false, 0) => trf("None of the {} is checked.", &[&what]),
                (true, n) if n <= 3 => trf("Unlocks the {} you're missing: {}.", &[&entries, &list(&names(&|_| true), 3)]),
                (true, _) => trf("Unlocks the {} you're missing.", &[&entries]),
                (false, _) => trf("Locks all {} that are checked now.", &[&entries]),
            };
            if tab == "3" {
                p.note = tr("Both of the game's award lists are written, as the game does.").into();
            }
            p.review = if all { trf("{}: all checked", &[&cap(what)]) } else { trf("{}: all unchecked", &[&cap(what)]) };
            p.count = entries;
            p.detail = tr("Collections").into();
        }
        _ => {}
    }
    if let Some(d) = GOALS.iter().find(|g| g.id == id) {
        p.conf = Some(d.conf);
    }
    if p.review.is_empty() {
        p.review = p.title.clone();
    }
    p
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// "11 bytes: base + 0x2817 (4), base + 0x282B (16)" — changed ranges of the file.
fn tech(changed: &[usize], base: usize) -> String {
    let mut ranges: Vec<(usize, usize)> = vec![];
    for &a in changed {
        match ranges.last_mut() {
            Some(r) if r.0 + r.1 == a => r.1 += 1,
            _ => ranges.push((a, 1)),
        }
    }
    let at = |a: usize| if a >= base && a < base + 0x11F8C4 { format!("base + 0x{:X}", a - base) } else { format!("0x{a:06X}") };
    let shown: Vec<String> = ranges.iter().take(6).map(|&(a, n)| format!("{} ({n})", at(a))).collect();
    let rest = ranges.len().saturating_sub(6);
    let more = if rest > 0 { trn(", and {} more range", ", and {} more ranges", rest as i64, &[&num(rest as i64)]) } else { String::new() };
    if changed.is_empty() {
        String::new()
    } else {
        let bytes = trn("{} byte", "{} bytes", changed.len() as i64, &[&num(changed.len() as i64)]);
        trf("{} at {}{}. The Save map names every range.", &[&bytes, &shown.join(", "), &more])
    }
}
