//! What an edit changes, in game terms: the value it touches, its name and value as the
//! UI shows them, and the bytes that hold it, so one value can go back to the file's
//! value on its own (a field's Undo, also inside a bulk edit).

use crate::assets;
use crate::fmt::{num, playtime};
use mhgu_save::character as ch;
use mhgu_save::data::tables;
use mhgu_save::equipment::{self, Owner};
use mhgu_save::items::{self, Store};
use mhgu_save::progress::{self as pg, Char, DEVIANTS, VILLAGES};
use mhgu_save::{monsters, palico, Save};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mon {
    Hunts,
    Captures,
    Min,
    Max,
    Notes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pal {
    Name,
    Level,
    Exp,
    Bias,
    Greeting,
    Owner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Name,
    Hr,
    HrPoints,
    Funds,
    Wycademy,
    Playtime,
    VillageStar,
    HubStar,
    /// Village, G rank.
    Points(usize, bool),
    Item(Store, usize),
    Equip(Owner, usize),
    Palico(usize, Pal),
    /// Quest index: its cleared, seen and failed bits.
    Quest(usize),
    /// Request index: accepted and completed flags.
    Request(usize),
    Art(u32),
    Dish(usize),
    Ingredient(usize),
    Award(usize),
    Permits(usize),
    /// Special Permit levels cleared of a deviant.
    Levels(usize),
    Monster(usize, Mon),
}

/// Pages in nav order; `Target::page` returns one of these ids.
pub const PAGES: [(&str, &str); 10] = [
    ("overview", "Overview"),
    ("character", "Character"),
    ("items", "Items"),
    ("equipment", "Equipment"),
    ("palicoes", "Palicoes"),
    ("quests", "Quests"),
    ("requests", "Requests"),
    ("collections", "Collections"),
    ("monsters", "Monsters"),
    ("advanced", "Save map"),
];

pub fn page_index(id: &str) -> usize {
    PAGES.iter().position(|p| p.0 == id).unwrap_or(0)
}

fn mon_field(f: Mon) -> &'static str {
    match f {
        Mon::Hunts => "hunts",
        Mon::Captures => "captures",
        Mon::Min => "min",
        Mon::Max => "max",
        Mon::Notes => "notes",
    }
}

fn pal_field(f: Pal) -> &'static str {
    match f {
        Pal::Name => "name",
        Pal::Level => "level",
        Pal::Exp => "exp",
        Pal::Bias => "bias",
        Pal::Greeting => "greeting",
        Pal::Owner => "owner",
    }
}

pub fn pal_of(field: &str) -> Pal {
    match field {
        "name" => Pal::Name,
        "level" => Pal::Level,
        "exp" => Pal::Exp,
        "bias" => Pal::Bias,
        "greeting" => Pal::Greeting,
        _ => Pal::Owner,
    }
}

fn store_key(s: Store) -> &'static str {
    if s == Store::Pouch { "pouch" } else { "box" }
}

fn owner_key(o: Owner) -> &'static str {
    if o == Owner::Palico { "palico" } else { "hunter" }
}

/// Bytes of a field and the bits of each that belong to it.
type Footprint = Vec<(usize, u8)>;

fn range(a: usize, n: usize) -> Footprint {
    (a..a + n).map(|x| (x, 0xFF)).collect()
}

fn bit(base: usize, off: usize, i: usize) -> (usize, u8) {
    (base + off + i / 8, 1 << (i % 8))
}

/// The bits `first .. first + n` of the LSB-first field at `at`.
fn bits(at: usize, first: usize, n: usize) -> Footprint {
    let mut out: Footprint = vec![];
    for b in first..first + n {
        let (a, m) = (at + b / 8, 1u8 << (b % 8));
        match out.last_mut() {
            Some(last) if last.0 == a => last.1 |= m,
            _ => out.push((a, m)),
        }
    }
    out
}

impl Target {
    pub fn page(&self) -> &'static str {
        use Target::*;
        match self {
            Name | Hr | HrPoints | Funds | Wycademy | Playtime | VillageStar | HubStar | Points(..) => "character",
            Item(..) => "items",
            Equip(..) => "equipment",
            Palico(..) => "palicoes",
            Quest(_) => "quests",
            Request(_) => "requests",
            Art(_) | Dish(_) | Ingredient(_) | Award(_) | Permits(_) | Levels(_) => "collections",
            Monster(..) => "monsters",
        }
    }

    /// Stable id the UI uses for the field (Undo, highlight).
    pub fn key(&self) -> String {
        use Target::*;
        match *self {
            Name => "name".into(),
            Hr => "hr".into(),
            HrPoints => "hr-points".into(),
            Funds => "funds".into(),
            Wycademy => "wycademy".into(),
            Playtime => "playtime".into(),
            VillageStar => "village-star".into(),
            HubStar => "hub-star".into(),
            Points(v, g) => format!("points:{}{v}", if g { "g" } else { "lr" }),
            Item(s, i) => format!("item:{}:{i}", store_key(s)),
            Equip(o, i) => format!("equip:{}:{i}", owner_key(o)),
            Palico(i, f) => format!("palico:{i}:{}", pal_field(f)),
            Quest(i) => format!("quest:{i}"),
            Request(i) => format!("request:{i}"),
            Art(i) => format!("check:0:{i}"),
            Dish(i) => format!("check:1:{i}"),
            Ingredient(i) => format!("check:2:{i}"),
            Award(i) => format!("check:3:{i}"),
            Permits(d) => format!("permits:{d}"),
            Levels(d) => format!("levels:{d}"),
            Monster(i, f) => format!("mon:{i}:{}", mon_field(f)),
        }
    }

    /// The key the page highlights for it (a monster's row rather than its cell).
    pub fn row_key(&self) -> String {
        match *self {
            Target::Monster(i, _) => format!("mon:{i}"),
            Target::Levels(d) => format!("levels:{d}"),
            _ => self.key(),
        }
    }

    /// Its name in game terms: "Wycademy points", "Bherna · G rank points", "Arzuros · Hunted".
    pub fn label(&self, s: &Save, slot: usize) -> String {
        use Target::*;
        let t = tables();
        match *self {
            Name => "Name".into(),
            Hr => "Hunter Rank".into(),
            HrPoints => "HR points".into(),
            Funds => "Zenny".into(),
            Wycademy => "Wycademy points".into(),
            Playtime => "Play time".into(),
            VillageStar => "Village ★".into(),
            HubStar => "Hub ★".into(),
            Points(v, g) => format!("{} · {} points", VILLAGES[v], if g { "G rank" } else { "Low rank" }),
            Item(st, i) => format!("{} slot {}", if st == Store::Pouch { "Pouch" } else { "Item box" }, i + 1),
            Equip(o, i) => format!("{} slot {}", if o == Owner::Palico { "Palico box" } else { "Box" }, i + 1),
            Palico(i, f) => {
                let n = palico::get(s, s.base(slot), i).name;
                let what = match f {
                    Pal::Name => "Name",
                    Pal::Level => "Level",
                    Pal::Exp => "Experience",
                    Pal::Bias => "Forte",
                    Pal::Greeting => "Greeting",
                    Pal::Owner => "Original owner",
                };
                format!("{n} · {what}")
            }
            Quest(i) => t.quests.iter().find(|q| q.index == i).map(|q| q.name.clone()).unwrap_or_else(|| format!("Quest {i}")),
            Request(i) => t
                .requests
                .iter()
                .find(|r| r.index == i)
                .map(|r| if r.quest_name.is_empty() { format!("Delivery request {i}") } else { r.quest_name.clone() })
                .unwrap_or_else(|| format!("Request {i}")),
            Art(id) => t.arts.iter().find(|a| a.0 == id).map(|a| a.1.clone()).unwrap_or_default(),
            Dish(b) => t.canteen.iter().find(|c| c.0 == "dish" && c.1 == b).map(|c| c.2.clone()).unwrap_or_default(),
            Ingredient(b) => t.canteen.iter().find(|c| c.0 == "ingredient" && c.1 == b).map(|c| c.2.clone()).unwrap_or_default(),
            Award(b) => t.awards.iter().find(|a| a.0 == b).map(|a| a.2.clone()).unwrap_or_default(),
            Permits(d) => format!("{} · Special Permits", DEVIANTS[d]),
            Levels(d) => format!("{} · levels cleared", DEVIANTS[d]),
            Monster(i, f) => {
                let what = match f {
                    Mon::Hunts => "Hunted",
                    Mon::Captures => "Captured",
                    Mon::Min => "Smallest",
                    Mon::Max => "Largest",
                    Mon::Notes => "Hunter's Notes",
                };
                format!("{} · {what}", t.monsters[i - 1].name)
            }
        }
    }

    /// Its value as the UI shows it ("9,999,999", "Mega Potion ×10", "Cleared", "98 %").
    pub fn read(&self, s: &Save, slot: usize) -> String {
        use Target::*;
        let base = s.base(slot);
        let on = |off: usize, i: usize| s.bit(base + off, i);
        let yes = |v: bool, a: &str, b: &str| if v { a.to_string() } else { b.to_string() };
        match *self {
            Name => ch::get(s, base).name,
            Hr => num(ch::get(s, base).hr),
            HrPoints => num(s.u32(base + ch::HR_POINTS)),
            Funds => num(s.u32(base + ch::FUNDS)),
            Wycademy => num(s.u32(base + ch::WYCADEMY)),
            Playtime => playtime(s.u32(base + ch::PLAYTIME)),
            VillageStar => num(s.u16(base + pg::VIL_STAR)),
            HubStar => num(s.u16(base + pg::HUB_STAR)),
            Points(v, g) => num(s.u32(base + if g { ch::POINTS_G } else { ch::POINTS_LR } + 4 * v)),
            Item(st, i) => {
                let x = items::get(s, base, st, i);
                if x.is_empty() { "Empty".into() } else { format!("{} ×{}", assets::item_name(x.id), x.count) }
            }
            Equip(o, i) => crate::views::equip_value(o, &equipment::get(s, base, o, i)),
            Palico(i, f) => {
                let p = palico::get(s, base, i);
                match f {
                    Pal::Name => p.name,
                    Pal::Level => num(p.level),
                    Pal::Exp => num(p.exp),
                    Pal::Bias => palico::BIASES.get(p.bias as usize).copied().unwrap_or("?").into(),
                    Pal::Greeting => p.greeting,
                    Pal::Owner => p.owner,
                }
            }
            Quest(i) => {
                if on(pg::CLEARED, i) {
                    "Cleared".into()
                } else if on(pg::FAILED, i) {
                    "Failed".into()
                } else {
                    yes(on(pg::SEEN, i), "Seen", "Not seen")
                }
            }
            Request(i) => {
                let r = tables().requests.iter().find(|r| r.index == i);
                let flag = |f: Option<usize>| f.is_some_and(|f| on(pg::FLAGS, f));
                match r {
                    Some(r) if flag(r.done_flag) => "Completed".into(),
                    Some(r) if flag(r.accept_flag) => "Accepted".into(),
                    _ => "Open".into(),
                }
            }
            Art(id) => yes(on(pg::ARTS, id as usize), "Unlocked", "Locked"),
            Dish(b) => yes(on(pg::DISHES, b), "Unlocked", "Locked"),
            Ingredient(b) => yes(on(pg::INGREDIENTS, b), "Unlocked", "Locked"),
            Award(b) => yes(on(pg::AWARDS_CARD, b), "Earned", "Not earned"),
            Permits(d) => num(s.u8(base + pg::PERMITS + d)),
            Levels(d) => {
                let (q0, n) = Char::deviant_levels(d);
                format!("{} / {n}", (0..n).filter(|&k| on(pg::CLEARED, q0 + k)).count())
            }
            Monster(i, f) => {
                let r = monsters::get(s, base, i);
                let pct = |v: u16| if v == 0 { "—".to_string() } else { format!("{v} %") };
                match f {
                    Mon::Hunts => num(r.hunts),
                    Mon::Captures => num(r.captures),
                    Mon::Min => pct(r.min),
                    Mon::Max => pct(r.max),
                    Mon::Notes => yes(monsters::notes(s, base, i).unwrap_or(false), "Unlocked", "Locked"),
                }
            }
        }
    }

    /// The bytes (and bits) that hold the value, its copies included.
    fn footprint(&self, s: &Save, slot: usize) -> Footprint {
        use Target::*;
        let base = s.base(slot);
        let hr_copies = || [range(base + ch::HDR_HR, 2), range(base + ch::CARD_HR, 2)].concat();
        match *self {
            Name => [range(base + ch::HDR_NAME, ch::NAME_LEN), range(base + ch::PLAYER_NAME, ch::NAME_LEN), range(base + ch::CARD, 2 * ch::CARD_NAME_UNITS)].concat(),
            Hr => [
                range(base + ch::HR_POINTS, 4),
                vec![bit(base, ch::PROGRESS, ch::HR_RELEASED_BIT), bit(base, ch::PROGRESS_NEW, ch::HR_RELEASED_BIT)],
                hr_copies(),
            ]
            .concat(),
            HrPoints => [range(base + ch::HR_POINTS, 4), hr_copies()].concat(),
            Funds => [range(base + ch::FUNDS, 4), range(base + ch::HDR_FUNDS, 4)].concat(),
            Wycademy => range(base + ch::WYCADEMY, 4),
            Playtime => [range(base + ch::PLAYTIME, 4), range(base + ch::HDR_PLAYTIME, 4), range(base + ch::CARD_PLAYTIME, 4)].concat(),
            VillageStar => range(base + pg::VIL_STAR, 2),
            HubStar => [range(base + pg::HUB_STAR, 2), hr_copies()].concat(),
            Points(v, g) => range(base + if g { ch::POINTS_G } else { ch::POINTS_LR } + 4 * v, 4),
            Item(st, i) => bits(base + if st == Store::Pouch { items::POUCH } else { items::BOX }, 19 * i, 19),
            Equip(o, i) => range(base + if o == Owner::Palico { equipment::PALICO_BOX } else { equipment::BOX } + equipment::ENTRY * i, equipment::ENTRY),
            Palico(i, f) => {
                let o = base + palico::LIST + palico::RECORD * i;
                match f {
                    Pal::Name => range(o, palico::NAME),
                    Pal::Level => range(o + palico::LEVEL, 1),
                    Pal::Exp => range(o + palico::EXP, 4),
                    Pal::Bias => range(o + palico::BIAS, 1),
                    Pal::Greeting => range(o + palico::GREETING.0, palico::GREETING.1),
                    Pal::Owner => range(o + palico::OWNER.0, palico::OWNER.1),
                }
            }
            Quest(i) => {
                let mut v = vec![bit(base, pg::CLEARED, i), bit(base, pg::SEEN, i), bit(base, pg::FAILED, i)];
                // the quest sets it completes
                if let Some(q) = tables().quests.iter().find(|q| q.index == i) {
                    v.extend(q.sets.iter().map(|&set| bit(base, pg::QUESTSETS, set as usize)));
                }
                v
            }
            Request(i) => {
                let t = tables();
                let mut flags: Vec<usize> = vec![];
                if let Some(r) = t.requests.iter().find(|r| r.index == i) {
                    flags.extend(r.accept_flag);
                    flags.extend(r.done_flag);
                }
                if let Some(o) = t.offers.iter().find(|o| o.index == i) {
                    flags.push(o.accept_flag);
                    flags.extend(o.also.iter().map(|f| f.unsigned_abs() as usize));
                }
                flags.into_iter().map(|f| bit(base, pg::FLAGS, f)).collect()
            }
            Art(id) => vec![bit(base, pg::ARTS, id as usize)],
            Dish(b) => vec![bit(base, pg::DISHES, b)],
            Ingredient(b) => vec![bit(base, pg::INGREDIENTS, b)],
            Award(b) => vec![bit(base, pg::AWARDS_CARD, b), bit(base, pg::AWARDS_GAME, b)],
            Permits(d) => range(base + pg::PERMITS + d, 1),
            Levels(d) => {
                let (q0, n) = Char::deviant_levels(d);
                (q0..q0 + n).flat_map(|q| [bit(base, pg::CLEARED, q), bit(base, pg::SEEN, q)]).collect()
            }
            Monster(i, f) => match f {
                Mon::Hunts => range(base + monsters::HUNTS + 2 * i, 2),
                Mon::Captures => range(base + monsters::CAPTURES + 2 * i, 2),
                Mon::Min => range(base + monsters::SIZES + 4 * i, 2),
                Mon::Max => range(base + monsters::SIZES + 4 * i + 2, 2),
                Mon::Notes => match monsters::meta(i).notes_bit {
                    Some(b) => vec![bit(base, monsters::NOTES, b), bit(base, monsters::NOTES_NEW, b)],
                    None => vec![],
                },
            },
        }
    }

    /// Put the file's value back, then redo what depends on it (HR copies, the Guild
    /// Card monster log).
    pub fn restore(&self, live: &mut Save, orig: &Save, slot: usize) {
        for (a, m) in self.footprint(live, slot) {
            let v = (live.u8(a) & !m) | (orig.u8(a) & m);
            if v != live.u8(a) {
                live.set_u8(a, v);
            }
        }
        let base = live.base(slot);
        match *self {
            Target::Hr | Target::HrPoints | Target::HubStar => {
                let p = live.u32(base + ch::HR_POINTS);
                ch::set_hr_points(live, base, p);
            }
            Target::Monster(i, _) => monsters::sync_card(live, base, i),
            _ => {}
        }
    }
}
