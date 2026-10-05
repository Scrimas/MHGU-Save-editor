//! Quick goals and page-level bulk actions: edits that write what playing would have
//! written, planned on a copy of the save and described in game terms (03, R3, R4).

use crate::fmt::{count, list, num};
use crate::state::Conf;
use crate::targets::{Mon, Target};
use mhgu_save::character as ch;
use mhgu_save::data::tables;
use mhgu_save::items::{self, Stack, Store};
use mhgu_save::progress::{Char, QuestBit, VILLAGES};
use mhgu_save::save::Save;
use mhgu_save::monsters;

pub struct Def {
    pub id: &'static str,
    pub conf: Conf,
}

/// The Overview's goals (titles and copy come from `plan`). Confirmed: tested in game
/// (arts and canteen as written, quests by the bulk completion of 2026-09-19); the rest
/// is derived from the game's code.
pub const GOALS: [Def; 8] = [
    Def { id: "quests", conf: Conf::Confirmed },
    Def { id: "arts", conf: Conf::Confirmed },
    Def { id: "canteen", conf: Conf::Confirmed },
    Def { id: "awards", conf: Conf::Derived },
    Def { id: "notes", conf: Conf::Derived },
    Def { id: "crowns", conf: Conf::Derived },
    Def { id: "hr999", conf: Conf::Derived },
    Def { id: "money", conf: Conf::Derived },
];

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
                let (nlo, nhi) = (if r.min == 0 { lo } else { r.min.min(lo) }, r.max.max(hi));
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
        ["items", what, _] => {
            let st = store(id);
            let v = items::all(s, base, st);
            let n: Vec<Stack> = match what {
                "sort" => items::compact(&v, true),
                "max" => v.iter().map(|x| if x.is_empty() { *x } else { Stack { id: x.id, count: items::MAX_COUNT } }).collect(),
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
    let mut p = Plan { action: "Add to Review".into(), detail: "Quick goal".into(), ..Default::default() };
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
                if let Target::Quest(i) = t {
                    if let Some(x) = tables().quests.iter().find(|x| x.index == *i) {
                        *by_cat.entry(x.category.clone()).or_default() += 1;
                    }
                }
            }
            let cats = by_cat.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
            p.title = "Complete every quest".into();
            p.summary = if q == 0 && r == 0 {
                "Every quest is cleared.".into()
            } else if q == 0 {
                format!("Every quest is cleared. Posts the {} their villagers would offer.", count(r, "villager request", "villager requests"))
            } else {
                format!(
                    "Clears the {} not cleared yet ({cats}){}.",
                    count(q, "quest", "quests"),
                    if r > 0 { format!(" and posts the {} their villagers would offer", count(r, "villager request", "villager requests")) } else { String::new() }
                )
            };
            p.count = count(q.max(r), if q > 0 { "quest" } else { "request" }, if q > 0 { "quests" } else { "requests" });
            p.note = "Star levels and story events stay to the game: they follow after your next quest. Completed quest sets are recorded too.".into();
        }
        ["arts"] => {
            p.title = "Unlock all Hunter Arts".into();
            p.summary = if n == 0 { "Every Hunter Art is unlocked.".into() } else { format!("Unlocks the {} you're missing: {}.", count(n, "Hunter Art", "Hunter Arts"), list(&names(&|_| true), 3)) };
            p.count = count(n, "art", "arts");
        }
        ["canteen"] => {
            let d = p.targets.iter().filter(|t| matches!(t, Target::Dish(_))).count();
            let i = n - d;
            p.title = "Canteen dishes and ingredients".into();
            p.summary = match (d, i) {
                (0, 0) => "Every Canteen dish and ingredient is unlocked.".into(),
                (d, 0) => format!("Unlocks the {} you're missing in the Canteen.", count(d, "dish", "dishes")),
                (0, i) => format!("Unlocks the {} you're missing in the Canteen.", count(i, "ingredient", "ingredients")),
                (d, i) => format!("Unlocks the {} and {} you're missing in the Canteen.", count(d, "dish", "dishes"), count(i, "ingredient", "ingredients")),
            };
            p.count = count(n, "entry", "entries");
        }
        ["awards"] => {
            p.title = "All Guild Card awards".into();
            p.summary = if n == 0 {
                "Your Guild Card has every award an edit can give.".into()
            } else {
                format!("Unlocks the {} you're missing: {}.", count(n, "award", "awards"), list(&names(&|_| true), 3))
            };
            p.count = count(n, "award", "awards");
            p.note = "Veteran Hunter's Prize comes only from a save transfer and stays as it is. Both of the game's award lists are written, as the game does.".into();
        }
        ["notes"] => {
            p.title = "Hunter's Notes".into();
            let mons: Vec<String> = p.targets.iter().filter_map(|t| if let Target::Monster(i, _) = t { Some(tables().monsters[i - 1].name.clone()) } else { None }).collect();
            p.summary = if n == 0 { "Every Hunter's Notes page is unlocked.".into() } else { format!("Unlocks the {} you're missing: {}.", count(n, "Hunter's Notes page", "Hunter's Notes pages"), list(&mons, 3)) };
            p.count = count(n, "page", "pages");
            p.note = "Each page gets its NEW mark, as the game's own unlock does.".into();
        }
        ["crowns"] => {
            let gold = p.targets.iter().filter(|t| matches!(t, Target::Monster(_, Mon::Max))).count();
            let mini = p.targets.iter().filter(|t| matches!(t, Target::Monster(_, Mon::Min))).count();
            p.title = "Every crown".into();
            p.summary = if n == 0 {
                "Every large monster has its mini and gold crown.".into()
            } else {
                let mut parts = vec![];
                if gold > 0 {
                    parts.push(format!("Gold crown records for the {} without one", count(gold, "monster", "monsters")));
                }
                if mini > 0 {
                    parts.push(format!("{} records for the {mini} without one", if gold > 0 { "mini crown" } else { "Mini crown" }));
                }
                format!("{}. Also rebuilds the Guild Card monster log.", parts.join(", and "))
            };
            p.count = count(n, "record", "records");
            p.note = "The crown awards follow after your next quest in game.".into();
            p.detail = "Quick goal · also rebuilds the Guild Card monster log".into();
        }
        ["hr999"] => {
            p.title = "HR 999".into();
            let st = ch::get(s, base);
            if s.u16(base + ch::HUB_STAR) == 0 {
                p.blocked = true;
                p.summary = "Join the Hub in game first: until then HR follows the Hub's star level.".into();
                p.count = "Not available yet".into();
            } else if n == 0 {
                p.summary = "HR is already 999.".into();
            } else {
                p.summary = format!("Raises HR {} to 999 and releases the HR limit, as the game does at 4,246,430 HR points.", num(st.hr));
                p.count = "HR 999".into();
            }
        }
        ["money"] => {
            p.title = "Max zenny and points".into();
            let has = |t: Target| p.targets.contains(&t);
            let vills = |g: bool| -> Vec<String> { (0..4).filter(|&v| has(Target::Points(v, g))).map(|v| VILLAGES[v].to_string()).collect() };
            let (lr, g) = (vills(false), vills(true));
            let mut doing = vec![];
            let mut done = vec![];
            for (t, what) in [(Target::Funds, "zenny"), (Target::Wycademy, "Wycademy points")] {
                if has(t) { doing.push(format!("{what} to 9,999,999")) } else { done.push(what.to_string()) }
            }
            for (v, rank) in [(&g, "G rank"), (&lr, "Low rank")] {
                match v.len() {
                    0 => done.push(format!("{rank} points")),
                    4 => doing.push(format!("{rank} village points to 20,000 in all four villages")),
                    _ => doing.push(format!("{rank} points to 20,000 in {}", list(v, 4))),
                }
            }
            let mut first = list(&doing, 4);
            if let Some(c0) = first.get(0..1) {
                first = c0.to_uppercase() + &first[1..];
            }
            let done_txt = match done.len() {
                0 => String::new(),
                1 => format!("{} is already at the max", cap(&done[0])),
                _ => format!("{} are already at the max", cap(&list(&done, 4))),
            };
            p.summary = if doing.is_empty() { "Zenny and every point total are at the max.".into() } else if done.is_empty() { format!("{first}.") } else { format!("{first}. {done_txt}.") };
            if !done_txt.is_empty() && !doing.is_empty() {
                p.note = format!("{done_txt} and stay as they are.");
            }
            p.count = count(n, "value", "values");
        }
        ["monsters", "hunts"] => {
            p.title = "Max hunts".into();
            p.summary = if n == 0 { "Every monster is hunted 9,999 times.".into() } else { format!("Sets Hunted to 9,999 for the {} below it. Also rebuilds the Guild Card monster log.", count(n, "monster", "monsters")) };
            p.count = count(n, "monster", "monsters");
            p.review = "Every monster hunted 9,999 times".into();
            p.detail = "Monsters · also rebuilds the Guild Card monster log".into();
        }
        ["items", what, _] => {
            let st = store(id);
            let place = if st == Store::Pouch { "pouch" } else { "item box" };
            let stacks = items::all(s, base, st).iter().filter(|x| !x.is_empty()).count();
            match what {
                "sort" => {
                    p.title = format!("Sort the {place}");
                    p.summary = if n == 0 { format!("The {place} is already sorted.") } else { format!("Sorts the {place} by item and merges stacks of the same item; {} change.", count(n, "slot", "slots")) };
                    p.review = format!("{}: sorted and merged", cap(place));
                }
                "max" => {
                    p.title = "Max counts".into();
                    p.summary = if n == 0 { format!("Every stack in the {place} is at 99.") } else { format!("Raises the {} below 99 in the {place} to 99.", count(n, "stack", "stacks")) };
                    p.review = format!("{}: every stack ×99", cap(place));
                }
                _ => {
                    p.title = format!("Empty the {place}");
                    p.summary = if stacks == 0 { format!("The {place} is already empty.") } else { format!("Removes all {} from the {place}. You can undo it in Review until you write.", count(stacks, "stack", "stacks")) };
                    p.review = format!("{} emptied: {}", cap(place), count(stacks, "stack", "stacks"));
                    p.action = if st == Store::Pouch { "Empty pouch".into() } else { "Empty box".into() };
                }
            }
            p.count = count(n, "slot", "slots");
            p.detail = cap(place);
        }
        ["checks", on, tab] => {
            let what = match tab {
                "0" => "Hunter Arts",
                "1" => "Canteen dishes",
                "2" => "Canteen ingredients",
                _ => "awards",
            };
            let all = on == "all";
            p.title = format!("{} every {}", if all { "Check" } else { "Uncheck" }, what.trim_end_matches('s'));
            p.summary = match (all, n) {
                (true, 0) => format!("Every one of the {what} is already checked."),
                (false, 0) => format!("None of the {what} is checked."),
                (true, n) if n <= 3 => format!("Unlocks the {} you're missing: {}.", count(n, "entry", "entries"), list(&names(&|_| true), 3)),
                (true, n) => format!("Unlocks the {} you're missing.", count(n, "entry", "entries")),
                (false, n) => format!("Locks all {} that are checked now.", count(n, "entry", "entries")),
            };
            if tab == "3" {
                p.note = "Both of the game's award lists are written, as the game does.".into();
            }
            p.review = format!("{}: all {}", cap(what), if all { "checked" } else { "unchecked" });
            p.count = count(n, "entry", "entries");
            p.detail = "Collections".into();
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
    let more = if ranges.len() > 6 { format!(", and {} more ranges", ranges.len() - 6) } else { String::new() };
    if changed.is_empty() {
        String::new()
    } else {
        format!("{} at {}{more}. The Save map names every range.", count(changed.len(), "byte", "bytes"), shown.join(", "))
    }
}
