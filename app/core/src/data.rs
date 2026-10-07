//! The repo's machine-readable tables (../../data/*.csv), compiled in.

use std::collections::HashMap;
use std::sync::OnceLock;

fn rows(src: &'static str) -> Vec<HashMap<String, String>> {
    let mut r = csv::Reader::from_reader(src.as_bytes());
    let head: Vec<String> = r.headers().unwrap().iter().map(str::to_owned).collect();
    r.records()
        .map(|rec| {
            let rec = rec.unwrap();
            head.iter().cloned().zip(rec.iter().map(str::to_owned)).collect()
        })
        .collect()
}

/// A number cell; empty is 0. A typo in a table fails debug builds (and `tables_load`)
/// instead of reading flag or bit 0.
fn num(s: &str) -> i64 {
    let s = s.trim();
    let v = if let Some(h) = s.strip_prefix("0x") { i64::from_str_radix(h, 16).ok() } else { s.parse().ok() };
    debug_assert!(v.is_some() || s.is_empty(), "not a number in data/*.csv: {s:?}");
    v.unwrap_or(0)
}

#[derive(Debug, Clone)]
pub struct Quest {
    pub index: usize,
    pub id: u32,
    pub category: String,
    pub prowler: bool,
    pub rank: String,
    pub name: String,
    pub group: u32,
    pub alt: u32,
    pub sets: Vec<u32>,
    pub notes: String,
}

impl Quest {
    /// Placeholder rows and unused event slots are not real quests.
    pub fn is_real(&self) -> bool {
        self.notes != "placeholder entry" && self.notes != "unused event slot"
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub index: usize,
    pub village: String,
    pub quest_id: Option<u32>,
    pub quest_name: String,
    pub accept_flag: Option<usize>,
    pub done_flag: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Offer {
    pub index: usize,
    pub quest_id: Option<u32>,
    pub npc_name: String,
    pub npc_bit: Option<usize>,
    pub talk_kind: String,
    pub accept_flag: usize,
    /// Flags the offer block also sets (positive) or clears (negative).
    pub also: Vec<i64>,
    pub offer: String,
}

#[derive(Debug, Clone)]
pub struct Monster {
    pub index: usize,
    pub name: String,
    pub large: bool,
    pub has_size: bool,
    pub confidence: String,
}

/// data/monster-sizes.csv: size table `em_size_scale_data` and the card / Notes maps.
#[derive(Debug, Clone)]
pub struct MonsterMeta {
    pub index: usize,
    /// Folded variant: its size record is never used, its counts add to this monster's.
    pub family_of: Option<usize>,
    pub size_record: bool,
    pub base_cm: Option<f32>,
    pub mini_le: u16,
    pub silver_ge: u16,
    pub gold_ge: u16,
    pub fixed_size: bool,
    pub crown_awards: bool,
    /// Position in the Guild Card monster log (87 entries).
    pub card_pos: Option<usize>,
    pub notes_bit: Option<usize>,
    pub class: String,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub abs: usize,
    pub size: usize,
    pub block: String,
    pub rel: usize,
    pub manager: String,
    pub label: String,
    pub confidence: String,
}

/// data/smithy-lists.csv: one Smithy list and the create-table records it can show.
#[derive(Debug, Clone)]
pub struct SmithyList {
    /// "weapon:<class>", "armor:<part>", "deco", "palico:<weapon|helm|mail>"
    pub list: String,
    pub records: usize,
    /// Armor: per record, how many armor ID slots it fills (1, 2 or 4).
    pub ids: Vec<u8>,
}

/// data/talisman-tables.csv: a skill a talisman tier can roll, or its most slots.
#[derive(Debug, Clone)]
pub struct TalismanRow {
    /// "skill1", "skill2" or "slots"
    pub kind: String,
    /// Talisman entry +0x12: 97 Mystery … 100 Enduring.
    pub tier: u8,
    pub skill: u8,
    pub min: i8,
    pub max: i8,
}

pub struct Tables {
    pub quests: Vec<Quest>,
    pub unlock: HashMap<u32, String>,
    pub rotating: HashMap<u32, usize>,
    pub requests: Vec<Request>,
    pub offers: Vec<Offer>,
    pub npc_bit: HashMap<String, usize>,
    pub monsters: Vec<Monster>,
    /// Indexed by monster index - 1.
    pub monster_meta: Vec<MonsterMeta>,
    pub arts: Vec<(u32, String)>,
    /// (kind, bit, name, category)
    pub canteen: Vec<(String, usize, String, String)>,
    /// (bit, grid, name)
    pub awards: Vec<(usize, String, String)>,
    pub fields: Vec<Field>,
    pub smithy: Vec<SmithyList>,
    pub talisman: Vec<TalismanRow>,
    /// data/quest-sizes.csv: monster index -> (smallest, largest) size % its quests give.
    pub quest_sizes: HashMap<usize, (u16, u16)>,
}

impl Tables {
    /// Quest index of the first row with this ID (the game counts a repeated ID once).
    pub fn quest_index(&self, id: u32) -> Option<usize> {
        self.quests.iter().find(|q| q.id == id).map(|q| q.index)
    }
}

pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let opt = |s: &String| (!s.is_empty()).then(|| num(s));
        Tables {
            quests: rows(include_str!("../../../data/quest-index.csv"))
                .iter()
                .map(|r| Quest {
                    index: num(&r["index"]) as usize,
                    id: num(&r["quest_id"]) as u32,
                    category: r["category"].clone(),
                    prowler: r["prowler"] == "yes",
                    rank: r["rank"].clone(),
                    name: r["name"].clone(),
                    group: num(&r["group"]) as u32,
                    alt: num(&r["alt"]) as u32,
                    sets: r["sets"].split_whitespace().map(|s| num(s) as u32).collect(),
                    notes: r["notes"].clone(),
                })
                .collect(),
            unlock: rows(include_str!("../../../data/quest-unlock.csv")).iter().map(|r| (num(&r["quest_id"]) as u32, r["rule"].clone())).collect(),
            rotating: rows(include_str!("../../../data/rotating-quests.csv")).iter().map(|r| (num(&r["quest_id"]) as u32, num(&r["bit"]) as usize)).collect(),
            requests: rows(include_str!("../../../data/request-index.csv"))
                .iter()
                .map(|r| Request {
                    index: num(&r["index"]) as usize,
                    village: r["village"].clone(),
                    quest_id: opt(&r["quest_id"]).map(|v| v as u32),
                    quest_name: r["quest_name"].clone(),
                    accept_flag: opt(&r["accept_flag"]).map(|v| v as usize),
                    done_flag: opt(&r["done_flag"]).map(|v| v as usize),
                })
                .collect(),
            offers: rows(include_str!("../../../data/request-offer.csv"))
                .iter()
                .map(|r| Offer {
                    index: num(&r["index"]) as usize,
                    quest_id: opt(&r["quest_id"]).map(|v| v as u32),
                    npc_name: r["npc_name"].clone(),
                    npc_bit: opt(&r["npc_bit"]).map(|v| v as usize),
                    talk_kind: r["talk_kind"].clone(),
                    accept_flag: num(&r["accept_flag"]) as usize,
                    also: r["also"].split_whitespace().map(num).collect(),
                    offer: r["offer"].clone(),
                })
                .collect(),
            npc_bit: rows(include_str!("../../../data/npc-index.csv")).iter().map(|r| (r["npc_id"].clone(), num(&r["bit"]) as usize)).collect(),
            monsters: rows(include_str!("../../../data/monster-index.csv"))
                .iter()
                .map(|r| Monster {
                    index: num(&r["index"]) as usize,
                    name: r["monster"].clone(),
                    large: r["category"] == "large",
                    has_size: r["has_size_record"] == "yes",
                    confidence: r["confidence"].clone(),
                })
                .collect(),
            monster_meta: rows(include_str!("../../../data/monster-sizes.csv"))
                .iter()
                .map(|r| MonsterMeta {
                    index: num(&r["index"]) as usize,
                    family_of: opt(&r["family_of"]).map(|v| v as usize),
                    size_record: r["size_record"] == "yes",
                    base_cm: r["base_cm"].parse().ok(),
                    mini_le: num(&r["mini_le"]) as u16,
                    silver_ge: num(&r["silver_ge"]) as u16,
                    gold_ge: num(&r["gold_ge"]) as u16,
                    fixed_size: r["fixed_size"] == "yes",
                    crown_awards: r["crown_awards"] == "yes",
                    card_pos: opt(&r["card_pos"]).map(|v| v as usize),
                    notes_bit: opt(&r["notes_bit"]).map(|v| v as usize),
                    class: r["class"].clone(),
                })
                .collect(),
            arts: rows(include_str!("../../../data/hunter-arts.csv")).iter().map(|r| (num(&r["id"]) as u32, r["name"].clone())).collect(),
            canteen: rows(include_str!("../../../data/canteen.csv"))
                .iter()
                .map(|r| (r["kind"].clone(), num(&r["bit"]) as usize, r["name"].clone(), r["category"].clone()))
                .collect(),
            awards: rows(include_str!("../../../data/awards.csv")).iter().map(|r| (num(&r["bit"]) as usize, r["grid"].clone(), r["name"].clone())).collect(),
            fields: rows(include_str!("../../../data/save-map.csv"))
                .iter()
                .map(|r| Field {
                    abs: num(&r["offset"]) as usize,
                    size: num(&r["size"]) as usize,
                    block: r["block"].clone(),
                    rel: num(&r["rel"]) as usize,
                    manager: r["manager"].clone(),
                    label: r["label"].clone(),
                    confidence: r["confidence"].clone(),
                })
                .collect(),
            smithy: rows(include_str!("../../../data/smithy-lists.csv"))
                .iter()
                .map(|r| SmithyList {
                    list: r["list"].clone(),
                    records: num(&r["records"]) as usize,
                    ids: r["ids"].bytes().map(|b| b - b'0').collect(),
                })
                .collect(),
            talisman: rows(include_str!("../../../data/talisman-tables.csv"))
                .iter()
                .map(|r| TalismanRow {
                    kind: r["kind"].clone(),
                    tier: num(&r["tier"]) as u8,
                    skill: num(&r["skill"]) as u8,
                    min: num(&r["min"]) as i8,
                    max: num(&r["max"]) as i8,
                })
                .collect(),
            quest_sizes: rows(include_str!("../../../data/quest-sizes.csv"))
                .iter()
                .map(|r| (num(&r["index"]) as usize, (num(&r["min"]) as u16, num(&r["max"]) as u16)))
                .collect(),
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn tables_load() {
        let t = super::tables();
        assert_eq!(t.quests.len(), 1508);
        assert_eq!(t.monsters.len(), 137);
        assert!(t.fields.len() > 800);
        assert_eq!(t.quest_index(101), Some(1));
        assert_eq!(t.smithy.len(), 23);
        assert_eq!(t.talisman.iter().filter(|r| r.kind == "slots").count(), 4);
        assert_eq!(t.quest_sizes.get(&1), Some(&(88, 125)));
        assert!(t.smithy.iter().filter(|l| l.list.starts_with("armor:")).all(|l| l.ids.len() == l.records));
    }
}
