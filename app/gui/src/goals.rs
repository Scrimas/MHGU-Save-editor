//! Quick goals: bulk edits that write what playing would have written.

use mhgu_save::data::tables;
use mhgu_save::progress::{Char, QuestBit};
use mhgu_save::save::Save;
use mhgu_save::{character, monsters};

pub struct Def {
    pub id: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
}

pub const ALL: [Def; 8] = [
    Def { id: "quests", title: "Complete every quest", detail: "Cleared and seen bits of every real quest, the villager requests' accepted flags and the quest set bits. Star levels and story flags stay to the game." },
    Def { id: "arts", title: "Unlock all Hunter Arts", detail: "All 178 arts, as the full mask at base + 0x2C13 (tested in game)." },
    Def { id: "canteen", title: "All Canteen dishes and ingredients", detail: "45 ingredients and 99 dishes (tested in game)." },
    Def { id: "awards", title: "All Guild Card awards", detail: "Bits 0-99 and 102-131 in the card and the game's own map; bits 100-101 are the two variants of one slot and stay." },
    Def { id: "notes", title: "Complete the Hunter's Notes", detail: "Every monster page, with its NEW mark, as the game's talk action sets them." },
    Def { id: "crowns", title: "Every crown", detail: "Mini and gold crown size records for every large monster that has them, and the Guild Card log rebuilt." },
    Def { id: "hr999", title: "HR 999", detail: "Releases the HR limit and sets 4,246,430 HR points; the game derives HR from them." },
    Def { id: "money", title: "Max zenny and points", detail: "9,999,999 zenny and Wycademy points, 20,000 points in every village (low and G rank)." },
];

/// Apply goal `id` and return a description of what changed.
pub fn apply(id: &str, s: &mut Save, slot: usize) -> Vec<String> {
    let base = s.base(slot);
    let mut out = vec![];
    match id {
        "quests" => {
            let mut c = Char::new(s, slot);
            let todo: Vec<usize> = Char::real_quests(true).iter().filter(|q| !c.quest(QuestBit::Cleared, q.index)).map(|q| q.index).collect();
            let mut by_cat = std::collections::BTreeMap::<String, usize>::new();
            for q in Char::real_quests(true).iter().filter(|q| todo.contains(&q.index)) {
                *by_cat.entry(q.category.clone()).or_default() += 1;
            }
            out.push(format!("{} quests cleared and seen: {}", todo.len(), by_cat.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")));
            let sets = c.clear_quests(&todo);
            if !sets.is_empty() {
                out.push(format!("Quest set bits set: {}", sets.iter().map(u32::to_string).collect::<Vec<_>>().join(" ")));
            }
            let mut n = 0;
            let mut seen = std::collections::HashSet::new();
            for o in &tables().offers {
                if o.quest_id.is_none() || !seen.insert(o.index) || c.flag(o.accept_flag) {
                    continue;
                }
                c.accept_request(o.index);
                n += 1;
            }
            out.push(format!("{n} villager requests marked accepted (their quests are on the board; the NPC hands out the reward when you report)"));
            out.push(format!("Village ★{} and Hub ★{} unchanged: the game raises them after the next clear", c.village_star(), c.hub_star()));
        }
        "arts" => {
            let mut c = Char::new(s, slot);
            let ids: Vec<u32> = tables().arts.iter().map(|a| a.0).filter(|&i| !c.art(i)).collect();
            for &i in &ids {
                c.set_art(i, true);
            }
            out.push(format!("{} Hunter Arts unlocked", ids.len()));
        }
        "canteen" => {
            let mut c = Char::new(s, slot);
            let (mut a, mut b) = (0, 0);
            for i in 0..45 {
                if !c.ingredient(i) {
                    c.set_ingredient(i, true);
                    a += 1;
                }
            }
            for i in 0..99 {
                if !c.dish(i) {
                    c.set_dish(i, true);
                    b += 1;
                }
            }
            out.push(format!("{a} ingredients and {b} dishes learned"));
        }
        "awards" => {
            let mut c = Char::new(s, slot);
            let mut n = 0;
            for b in (0..100).chain(102..132) {
                if !c.award(b) {
                    n += 1;
                }
                c.set_award(b, true);
            }
            out.push(format!("{n} awards earned (card and game map)"));
        }
        "notes" => {
            let mut n = 0;
            for i in 1..=monsters::N {
                if monsters::notes(s, base, i) == Some(false) {
                    monsters::set_notes(s, base, i, true);
                    n += 1;
                }
            }
            out.push(format!("{n} Hunter's Notes pages unlocked"));
        }
        "crowns" => {
            let mut n = 0;
            for i in 1..=monsters::N {
                let Some((lo, hi)) = monsters::crown_sizes(i) else { continue };
                let mut r = monsters::get(s, base, i);
                let (nlo, nhi) = (if r.min == 0 { lo } else { r.min.min(lo) }, r.max.max(hi));
                if (nlo, nhi) != (r.min, r.max) {
                    r.min = nlo;
                    r.max = nhi;
                    monsters::set(s, base, i, r);
                    n += 1;
                }
            }
            out.push(format!("{n} size records extended to the crown thresholds"));
            out.push("Awards 7, 8, 115 and 116 follow after the next quest".into());
        }
        "hr999" => {
            let before = character::get(s, base).hr;
            if before == 999 {
                return out;
            }
            if character::set_hr(s, base, 999) {
                out.push(format!("HR {before} → 999 (HR points 4,246,430, HR limit released)"));
            } else {
                out.push("The Hub is not joined yet (Hub ★0): HR stays 0".into());
            }
        }
        "money" => {
            character::set_funds(s, base, character::MAX_FUNDS);
            character::set_wycademy(s, base, character::MAX_POINTS);
            for v in 0..4 {
                character::set_village_points(s, base, v, false, character::MAX_VILLAGE_POINTS);
                character::set_village_points(s, base, v, true, character::MAX_VILLAGE_POINTS);
            }
            out.push("Zenny and Wycademy points 9,999,999; village points 20,000 × 8".into());
        }
        _ => {}
    }
    out
}

/// Number of bytes a goal would change (for the button label).
pub fn edits(id: &str, s: &Save, slot: usize) -> usize {
    let mut c = s.clone();
    apply(id, &mut c, slot);
    c.bytes().iter().zip(s.bytes()).filter(|(a, b)| a != b).count()
}
