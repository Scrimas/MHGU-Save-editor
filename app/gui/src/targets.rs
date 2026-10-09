//! What an edit changes, in game terms: the value it touches, its name and value as the
//! UI shows them, and the bytes that hold it, so one value can go back to the file's
//! value on its own (a field's Undo, also inside a bulk edit).

use crate::assets;
use crate::fmt::{arena_time, num, playtime};
use crate::i18n::{tr, trf, trn};
use mhgu_save::character as ch;
use mhgu_save::data::tables;
use mhgu_save::equipment::{self, Owner};
use mhgu_save::items::{self, Store};
use mhgu_save::progress::{self as pg, Char, DEVIANTS, VILLAGES};
use mhgu_save::guildcard as gc;
use mhgu_save::{arena, monsters, palico, save, sets, slots, smithy, unlocks, Save};

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
    Target,
    /// Its support move list and the moves equipped.
    Moves,
    /// Its skill list and the skills equipped.
    Skills,
    /// Coat, eyes, ears, tail, voice, clothing and their colours.
    Looks,
    /// The whole record and the Palico equipment box entries it wears (a Palico copied in).
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetPart {
    Name,
    /// The seven pieces with their decorations.
    Gear,
    Pigment,
    /// Hunting style, Hunter Arts and SP Arts.
    Arts,
}

pub const SET_PARTS: [SetPart; 4] = [SetPart::Name, SetPart::Gear, SetPart::Pigment, SetPart::Arts];

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
    /// Guild Card weapon usage: venue, weapon (storage order).
    WeaponUse(usize, usize),
    Item(Store, usize),
    /// An item loadout: its name and items.
    Loadout(usize),
    Equip(Owner, usize),
    Palico(usize, Pal),
    /// Quest index: its cleared, seen and failed bits.
    Quest(usize),
    /// Request index: accepted and reported flags.
    Request(usize),
    Art(u32),
    Dish(usize),
    Ingredient(usize),
    Award(usize),
    Permits(usize),
    /// Special Permit levels cleared of a deviant.
    Levels(usize),
    Monster(usize, Mon),
    /// The items-obtained map, as one value.
    Obtained,
    /// A Smithy list, by its index in `smithy::lists()`: listed and NEW bits.
    Smithy(usize),
    /// The own Guild Card's title (three words), scene and pose.
    Title,
    Scene,
    Pose,
    /// A Guild Card unlock map (index in `guildcard::MAPS`) with its NEW copy.
    CardMap(usize),
    /// An Arena quest of the counter's table: the card's times, the counter's best time
    /// and the sets it was cleared with.
    Arena(usize),
    /// The hunter's face, hairstyle, features, voice, clothing and colours.
    Appearance,
    /// The hunter's body type (gender).
    Gender,
    /// Guild Card quests completed of one category (`character::QUEST_KINDS`).
    CardQuests(usize),
    /// The Guild Card greeting.
    Greeting,
    /// Quests done with one hunting style.
    StyleUse(usize),
    /// A whole character slot (0-2, whatever the edit's character): copied, swapped,
    /// deleted or imported.
    Slot(usize),
    /// A part of a My Set.
    MySet(usize, SetPart),
    /// A Palico equipment set: name and gear.
    PalicoSet(usize),
    /// The armor pigment the hunter has on.
    Pigment,
    /// The hunting style and Hunter Arts the hunter has on.
    Arts,
    /// An unlock map (index in `unlocks::maps()`) with its NEW copy and the fields that
    /// follow it (Lab: offered and supply sets; Notes second list: the Notes NEW marks).
    Unlock(usize),
    /// A village pet (`unlocks::PETS`): name, costume worn, adoption.
    Pet(usize),
    /// The four Moofahs' affection.
    Moofahs,
}

/// Pages in nav order; `Target::page` returns one of these ids. The titles are English:
/// the UI shows `page_title`.
pub const PAGES: [(&str, &str); 12] = [
    ("overview", "Overview"),
    ("character", "Character"),
    ("items", "Items"),
    ("equipment", "Equipment"),
    ("palicoes", "Palicoes"),
    ("quests", "Quests"),
    ("requests", "Requests"),
    ("collections", "Collections"),
    ("unlocks", "Unlocks"),
    ("monsters", "Monsters"),
    ("database", "Database"),
    ("advanced", "Save map"),
];

pub fn page_index(id: &str) -> usize {
    PAGES.iter().position(|p| p.0 == id).unwrap_or(0)
}

/// Title of page `i` of PAGES in the language in use.
pub fn page_title(i: usize) -> &'static str {
    match PAGES.get(i).map_or("", |p| p.0) {
        "overview" => tr("Overview"),
        "character" => tr("Character"),
        "items" => tr("Items"),
        "equipment" => tr("Equipment"),
        "palicoes" => tr("Palicoes"),
        "quests" => tr("Quests"),
        "requests" => tr("Requests"),
        "collections" => tr("Collections"),
        "unlocks" => tr("Unlocks"),
        "monsters" => tr("Monsters"),
        "database" => tr("Database"),
        "advanced" => tr("Save map"),
        _ => "",
    }
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
        Pal::Target => "target",
        Pal::Moves => "moves",
        Pal::Skills => "skills",
        Pal::Looks => "looks",
        Pal::All => "all",
    }
}

pub fn pal_of(field: &str) -> Pal {
    match field {
        "name" => Pal::Name,
        "level" => Pal::Level,
        "exp" => Pal::Exp,
        "bias" => Pal::Bias,
        "greeting" => Pal::Greeting,
        "target" => Pal::Target,
        "moves" => Pal::Moves,
        "skills" => Pal::Skills,
        "looks" => Pal::Looks,
        "all" => Pal::All,
        _ => Pal::Owner,
    }
}

fn set_part(p: SetPart) -> &'static str {
    match p {
        SetPart::Name => "name",
        SetPart::Gear => "gear",
        SetPart::Pigment => "pigment",
        SetPart::Arts => "arts",
    }
}

fn store_key(s: Store) -> &'static str {
    if s == Store::Pouch { "pouch" } else { "box" }
}

fn owner_key(o: Owner) -> &'static str {
    if o == Owner::Palico { "palico" } else { "hunter" }
}

/// Entry `i` of a Guild Card name list, "#i" when the asset pack lacks it.
pub fn card_name(names: &[String], i: usize) -> String {
    names.get(i).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| format!("#{i}"))
}

/// "Titan Slayer", "Titan of Slayer": the words in the card's order, an empty linking
/// word (0) left out.
pub fn card_title(t: [u16; 3]) -> String {
    let n = assets::names();
    let mut v = vec![card_name(&n.gc_words, t[0] as usize)];
    if t[1] != 0 {
        v.push(card_name(&n.gc_links, t[1] as usize));
    }
    v.push(card_name(&n.gc_words, t[2] as usize));
    v.join(" ")
}

/// The body type as the Character page names it.
pub fn body_type(g: u8) -> String {
    match g {
        0 => tr("Type 1 (male)").into(),
        1 => tr("Type 2 (female)").into(),
        g => format!("{g}"),
    }
}

/// Name of Palico support move or skill `id`, "#id" when the asset pack lacks it.
pub fn palico_name(names: &[String], id: u8) -> String {
    names.get(id as usize).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| format!("#{id}"))
}

/// The equipped moves or skills of a Palico, "None" when it has none.
pub fn palico_names(names: &[String], on: &[u8]) -> String {
    let v: Vec<String> = on.iter().filter(|&&x| x != 0).map(|&x| palico_name(names, x)).collect();
    if v.is_empty() { tr("None").into() } else { v.join(", ") }
}

/// Name of Arena quest `q` of the counter's table.
pub fn arena_quest(q: usize) -> String {
    let id = arena::quests()[q].quest_id;
    tables().quests.iter().find(|x| x.id == id).map(|x| x.name.clone()).unwrap_or_else(|| trf("Quest {}", &[&id]))
}

/// A grade as the Arena Counter ranks it: A, B, C, or a dash past the slowest time.
pub fn arena_grade(g: u8) -> &'static str {
    match g {
        0 => "S",
        1 => "A",
        2 => "B",
        _ => "—",
    }
}

/// "Smithy · Great Sword", "Smithy · Head", "Palico smithy · Mail".
pub fn smithy_label(i: usize) -> String {
    let l = &smithy::lists()[i].list;
    let (kind, n) = l.split_once(':').unwrap_or((l, ""));
    let k: usize = n.parse().unwrap_or(0);
    match kind {
        "weapon" => trf("Smithy · {}", &[&crate::views::weapon_classes().get(k).copied().unwrap_or("?")]),
        "armor" => trf("Smithy · {}", &[&crate::views::armor_parts().get(k.wrapping_sub(1)).copied().unwrap_or("?")]),
        "deco" => tr("Smithy · Decorations").into(),
        _ => match n {
            "weapon" => tr("Palico smithy · Weapons").into(),
            "helm" => tr("Palico smithy · Helms").into(),
            _ => tr("Palico smithy · Mail").into(),
        },
    }
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
            Name | Hr | HrPoints | Funds | Wycademy | Playtime | VillageStar | HubStar | Points(..) | WeaponUse(..) | Title | Scene | Pose | CardMap(_) | Appearance | Gender | Greeting | StyleUse(_)
            | Pigment | Arts => "character",
            Item(..) | Loadout(_) | Obtained => "items",
            Equip(..) | Smithy(_) | MySet(..) | PalicoSet(_) => "equipment",
            Palico(..) => "palicoes",
            // the card's quest counts follow the cleared quests
            Quest(_) | Arena(_) | CardQuests(_) => "quests",
            Request(_) => "requests",
            Art(_) | Dish(_) | Ingredient(_) | Award(_) | Permits(_) | Levels(_) => "collections",
            Unlock(_) | Pet(_) | Moofahs => "unlocks",
            Monster(..) => "monsters",
            Slot(_) => "overview",
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
            WeaponUse(v, w) => format!("use:{v}:{w}"),
            Item(s, i) => format!("item:{}:{i}", store_key(s)),
            Loadout(k) => format!("loadout:{k}"),
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
            Obtained => "obtained".into(),
            Smithy(i) => format!("smithy:{i}"),
            Title => "card-title".into(),
            Scene => "card-scene".into(),
            Pose => "card-pose".into(),
            CardMap(m) => format!("card-map:{m}"),
            Arena(q) => format!("arena:{q}"),
            Appearance => "appearance".into(),
            Gender => "gender".into(),
            CardQuests(k) => format!("quests:{k}"),
            Greeting => "card-greeting".into(),
            StyleUse(k) => format!("style:{k}"),
            Slot(k) => format!("slot:{k}"),
            MySet(k, p) => format!("myset:{k}:{}", set_part(p)),
            PalicoSet(k) => format!("palset:{k}"),
            Pigment => "pigment".into(),
            Arts => "arts".into(),
            Unlock(m) => format!("unlock:{}", unlocks::maps()[m].id),
            Pet(k) => format!("pet:{k}"),
            Moofahs => "moofahs".into(),
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
            Name => tr("Name").into(),
            Hr => tr("Hunter Rank").into(),
            HrPoints => tr("HR points").into(),
            Funds => tr("Zenny").into(),
            Wycademy => tr("Wycademy points").into(),
            Playtime => tr("Play time").into(),
            VillageStar => tr("Village ★").into(),
            HubStar => tr("Hub ★").into(),
            Points(v, g) => if g { trf("{} · G rank points", &[&tr(VILLAGES[v])]) } else { trf("{} · Low rank points", &[&tr(VILLAGES[v])]) },
            WeaponUse(v, w) => trf("{} · {} quests", &[&tr(ch::USE_WEAPONS[w]), &tr(ch::VENUES[v])]),
            Item(st, i) => if st == Store::Pouch { trf("Pouch slot {}", &[&(i + 1)]) } else { trf("Item box slot {}", &[&(i + 1)]) },
            Loadout(k) => trf("Item loadout {}", &[&(k + 1)]),
            Equip(o, i) => if o == Owner::Palico { trf("Palico box slot {}", &[&(i + 1)]) } else { trf("Box slot {}", &[&(i + 1)]) },
            Palico(i, f) => {
                let n = palico::get(s, s.base(slot), i).name;
                let what = match f {
                    Pal::Name => tr("Name"),
                    Pal::Level => tr("Level"),
                    Pal::Exp => tr("Experience"),
                    Pal::Bias => tr("Forte"),
                    Pal::Greeting => tr("Greeting"),
                    Pal::Owner => tr("Original owner"),
                    Pal::Target => tr("Target"),
                    Pal::Moves => tr("Support moves"),
                    Pal::Skills => tr("Skills"),
                    Pal::Looks => tr("Looks"),
                    Pal::All => tr("Whole Palico"),
                };
                if n.is_empty() { trf("Palico place {} · {}", &[&(i + 1), &what]) } else { format!("{n} · {what}") }
            }
            Quest(i) => t.quests.iter().find(|q| q.index == i).map(|q| q.name.clone()).unwrap_or_else(|| trf("Quest {}", &[&i])),
            Request(i) => t
                .requests
                .iter()
                .find(|r| r.index == i)
                .map(|r| if r.quest_name.is_empty() { trf("Delivery request {}", &[&i]) } else { r.quest_name.clone() })
                .unwrap_or_else(|| trf("Request {}", &[&i])),
            Art(id) => t.arts.iter().find(|a| a.0 == id).map(|a| a.1.clone()).unwrap_or_default(),
            Dish(b) => t.canteen.iter().find(|c| c.0 == "dish" && c.1 == b).map(|c| c.2.clone()).unwrap_or_default(),
            Ingredient(b) => t.canteen.iter().find(|c| c.0 == "ingredient" && c.1 == b).map(|c| c.2.clone()).unwrap_or_default(),
            Award(b) => t.awards.iter().find(|a| a.0 == b).map(|a| a.2.clone()).unwrap_or_default(),
            Permits(d) => trf("{} · Special Permits", &[&tr(DEVIANTS[d])]),
            Levels(d) => trf("{} · levels cleared", &[&tr(DEVIANTS[d])]),
            Monster(i, f) => {
                let what = match f {
                    Mon::Hunts => tr("Hunted"),
                    Mon::Captures => tr("Captured"),
                    Mon::Min => tr("Smallest"),
                    Mon::Max => tr("Largest"),
                    Mon::Notes => tr("Hunter's Notes"),
                };
                format!("{} · {what}", crate::assets::monster_name(i).unwrap_or(&t.monsters[i - 1].name))
            }
            Obtained => tr("Items obtained").into(),
            Smithy(i) => smithy_label(i),
            Title => tr("Guild Card title").into(),
            Scene => tr("Guild Card scene").into(),
            Pose => tr("Guild Card pose").into(),
            CardMap(m) => match gc::MAPS[m] {
                gc::Map::Words => tr("Title words unlocked").into(),
                gc::Map::Links => tr("Title linking words unlocked").into(),
                gc::Map::Scenes => tr("Guild Card scenes unlocked").into(),
                gc::Map::Poses => tr("Guild Card poses unlocked").into(),
            },
            Arena(q) => trf("{} · Arena record", &[&arena_quest(q)]),
            Appearance => tr("Appearance").into(),
            Gender => tr("Body").into(),
            CardQuests(k) => trf("{} quests", &[&tr(ch::QUEST_KINDS[k])]),
            Greeting => tr("Guild Card greeting").into(),
            StyleUse(k) => trf("{} quests", &[&tr(ch::STYLES[k])]),
            Slot(k) => trf("Character slot {}", &[&(k + 1)]),
            MySet(k, p) => {
                let what = match p {
                    SetPart::Name => tr("Name"),
                    SetPart::Gear => tr("Gear"),
                    SetPart::Pigment => tr("Pigment"),
                    SetPart::Arts => tr("Style and Hunter Arts"),
                };
                let m = sets::my_set(s, s.base(slot), k);
                if m.used() { format!("{} · {what}", trf("My Set {} “{}”", &[&(k + 1), &m.name])) } else { format!("{} · {what}", trf("My Set {}", &[&(k + 1)])) }
            }
            PalicoSet(k) => trf("Palico set {}", &[&(k + 1)]),
            Pigment => tr("Armor pigment").into(),
            Arts => tr("Hunting style and Hunter Arts").into(),
            Unlock(m) => crate::views::unlock_map_name(m),
            Pet(k) => trf("{} pet", &[&tr(["Bherna", "Kokoto", "Pokke", "Yukumo"][k])]),
            Moofahs => tr("Moofah affection").into(),
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
            WeaponUse(v, w) => num(ch::weapon_use(s, base, v, w)),
            Item(st, i) => {
                let x = items::get(s, base, st, i);
                if x.is_empty() { tr("Empty").into() } else { format!("{} ×{}", assets::item_name(x.id), x.count) }
            }
            Loadout(k) => {
                let l = items::loadout(s, base, k);
                let n = l.items.iter().filter(|x| x.0 != 0).count();
                if l.name.is_empty() && n == 0 { tr("Empty").into() } else { format!("{} · {}", l.name, trn("{} item", "{} items", n as i64, &[&num(n as i64)])) }
            }
            Equip(o, i) => crate::views::equip_value(o, &equipment::get(s, base, o, i)),
            Palico(i, f) => {
                let p = palico::get(s, base, i);
                match f {
                    Pal::Name => p.name,
                    Pal::Level => num(p.level),
                    Pal::Exp => num(p.exp),
                    Pal::Bias => tr(palico::BIASES.get(p.bias as usize).copied().unwrap_or("?")).into(),
                    Pal::Greeting => p.greeting,
                    Pal::Owner => p.owner,
                    Pal::Target => tr(palico::TARGETS.get(p.target as usize).copied().unwrap_or("?")).into(),
                    Pal::Moves => palico_names(&assets::names().support_moves, &p.moves),
                    Pal::Skills => palico_names(&assets::names().palico_skills, &p.skills_on),
                    Pal::Looks => {
                        let coat = assets::names().palico_coats.get(p.looks[6] as usize).cloned().unwrap_or_else(|| trf("Type {}", &[&(p.looks[6] + 1)]));
                        let [r, g, b, _] = p.colours[0];
                        format!("{coat} · #{r:02x}{g:02x}{b:02x}")
                    }
                    Pal::All if palico::is_empty(s, base, i) => tr("No Palico").into(),
                    Pal::All => trf("{} · Lv {} · {}", &[&p.name, &p.level, &tr(palico::BIASES.get(p.bias as usize).copied().unwrap_or("?"))]),
                }
            }
            Quest(i) => {
                if on(pg::CLEARED, i) {
                    tr("Cleared").into()
                } else if on(pg::FAILED, i) {
                    tr("Failed").into()
                } else {
                    yes(on(pg::SEEN, i), tr("Seen"), tr("Not seen"))
                }
            }
            Request(i) => {
                let r = tables().requests.iter().find(|r| r.index == i);
                let flag = |f: Option<usize>| f.is_some_and(|f| on(pg::FLAGS, f));
                match r {
                    Some(r) if flag(r.done_flag) => tr("Reported").into(),
                    Some(r) if flag(r.accept_flag) => tr("Accepted").into(),
                    _ => tr("Open").into(),
                }
            }
            Art(id) => yes(on(pg::ARTS, id as usize), tr("Unlocked"), tr("Locked")),
            Dish(b) => yes(on(pg::DISHES, b), tr("Unlocked"), tr("Locked")),
            Ingredient(b) => yes(on(pg::INGREDIENTS, b), tr("Unlocked"), tr("Locked")),
            Award(b) => yes(on(pg::AWARDS_CARD, b), tr("Earned"), tr("Not earned")),
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
                    Mon::Notes => yes(monsters::notes(s, base, i).unwrap_or(false), tr("Unlocked"), tr("Locked")),
                }
            }
            Obtained => {
                let n = (1..=items::OBTAINED_MAX_ID).filter(|&id| items::obtained(s, base, id)).count() as i64;
                trn("{} item", "{} items", n, &[&num(n)])
            }
            Smithy(i) => {
                let (n, all) = smithy::count(s, base, &smithy::lists()[i], ch::get(s, base).gender);
                trf("{} of {} listed", &[&num(n as i64), &num(all as i64)])
            }
            Title => card_title(gc::title(s, base)),
            Scene => card_name(&assets::names().gc_scenes, gc::scene(s, base) as usize),
            Pose => card_name(&assets::names().gc_poses, gc::pose(s, base) as usize),
            CardMap(m) => {
                let (_, _, n) = gc::MAPS[m].at();
                let k = (0..n).filter(|&i| gc::unlocked(s, base, gc::MAPS[m], i)).count();
                trf("{} of {} unlocked", &[&num(k as i64), &num(n as i64)])
            }
            Arena(q) => match arena::best(s, base, q) {
                Some(e) => format!("{} · {}", arena_time(e.time), arena_grade(e.grade)),
                None => tr("No record").into(),
            },
            Appearance => {
                let l = |k| ch::look(s, base, k) as i32;
                trf("Face {} · Hairstyle {} · Voice {}", &[&(l(ch::LOOK_FACE) + 1), &(l(ch::LOOK_HAIR) + 1), &l(ch::LOOK_VOICE)])
            }
            Gender => body_type(ch::look(s, base, ch::LOOK_GENDER)),
            CardQuests(k) => num(ch::card_quests(s, base, k)),
            Greeting => ch::greeting(s, base),
            StyleUse(k) => num(ch::style_use(s, base, k)),
            Slot(k) if !s.slot_used(k) => tr("Empty").into(),
            Slot(k) => {
                let c = ch::get(s, s.base(k));
                trf("{} · HR {} · {}", &[&c.name, &num(c.hr), &playtime(c.playtime)])
            }
            MySet(k, p) => {
                let m = sets::my_set(s, base, k);
                match p {
                    SetPart::Name => m.name,
                    SetPart::Gear => {
                        let v: Vec<String> = m.gear.iter().filter(|&&g| g != sets::NO_BOX).map(|&g| crate::views::box_piece_value(s, base, Owner::Hunter, g)).collect();
                        if v.is_empty() { tr("None").into() } else { v.join(", ") }
                    }
                    SetPart::Pigment => crate::views::pigment_value(&m.pigment),
                    SetPart::Arts => crate::views::arts_value(&m.arts),
                }
            }
            PalicoSet(k) => {
                let p = sets::palico_set(s, base, k);
                if !p.used() {
                    return tr("Empty").into();
                }
                let v: Vec<String> = p.gear.iter().filter(|&&g| g != sets::NO_BOX).map(|&g| crate::views::box_piece_value(s, base, Owner::Palico, g)).collect();
                format!("{} · {}", p.name, if v.is_empty() { tr("None").to_string() } else { v.join(", ") })
            }
            Pigment => crate::views::pigment_value(&sets::pigment(s, base)),
            Arts => crate::views::arts_value(&sets::arts(s, base)),
            Unlock(m) => {
                let (k, n) = unlocks::count(s, base, m);
                trf("{} of {}", &[&num(k as i64), &num(n as i64)])
            }
            Pet(k) => {
                let c = crate::views::pet_costume_name(unlocks::pet_costume(s, base, k));
                let n = unlocks::pet_name(s, base, k);
                if unlocks::adopted(s, base, k) { trf("{} · {} · adopted", &[&n, &c]) } else { format!("{n} · {c}") }
            }
            Moofahs => (0..unlocks::PETS).map(|k| unlocks::moofah(s, base, k).to_string()).collect::<Vec<_>>().join(" · "),
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
            WeaponUse(v, w) => range(base + ch::WEAPON_USE + 2 * (15 * v + w), 2),
            Item(st, i) => bits(base + if st == Store::Pouch { items::POUCH } else { items::BOX }, 19 * i, 19),
            Loadout(k) => range(base + items::LOADOUTS + items::LOADOUT_SZ * k, items::LOADOUT_SZ),
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
                    Pal::Target => range(o + palico::TARGET, 1),
                    Pal::Moves => [range(o + palico::MOVES, 8), range(o + palico::LEARNED, 16)].concat(),
                    Pal::Skills => [range(o + palico::SKILLS_ON, 8), range(o + palico::SKILLS, 12)].concat(),
                    Pal::Looks => [range(o + palico::LOOKS, 12), range(o + palico::COLOURS, 36)].concat(),
                    // the entries it wears now: put back, they are free again
                    Pal::All => {
                        let worn = palico::gear(s, base, i).into_iter().map(|j| range(base + equipment::PALICO_BOX + equipment::ENTRY * j, equipment::ENTRY));
                        std::iter::once(range(o, palico::RECORD)).chain(worn).flatten().collect()
                    }
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
            Obtained => range(base + items::OBTAINED, 376),
            Smithy(i) => match smithy::at(&smithy::lists()[i].list) {
                Some((listed, new, n)) => [range(base + listed, n), range(base + new, n)].concat(),
                None => vec![],
            },
            Title => range(base + gc::TITLE, 6),
            Scene => range(base + gc::SCENE, 1),
            Pose => range(base + gc::POSE, 1),
            CardMap(m) => {
                let (on, new, n) = gc::MAPS[m].at();
                [bits(base + on, 0, n), bits(base + new, 0, n)].concat()
            }
            Arena(q) => [
                range(base + arena::LOG + 20 * q, 20),
                range(base + arena::PARTNERS + 8 * q, 8),
                range(base + arena::BEST + 12 * q, 12),
                bits(base + arena::SETS, 5 * q, 5),
                bits(base + arena::SETS_NEW, 5 * q, 5),
            ]
            .concat(),
            Appearance => ch::LOOKS
                .into_iter()
                .zip(ch::LOOK_COLOURS)
                .flat_map(|(a, c)| {
                    let bytes = [ch::LOOK_VOICE, ch::LOOK_FACE, ch::LOOK_CLOTHING, ch::LOOK_HAIR, ch::LOOK_FEATURES].map(|k| (base + a + k, 0xFF));
                    [bytes.to_vec(), range(base + c + 4 * ch::COLOUR_SKIN, 4), range(base + c + 4 * ch::COLOUR_CLOTHING, 4)].concat()
                })
                .collect(),
            Gender => ch::LOOKS.iter().map(|&a| (base + a + ch::LOOK_GENDER, 0xFF)).chain([(base + ch::HDR_GENDER, 0xFF)]).collect(),
            CardQuests(k) => range(base + ch::CARD_QUESTS + 2 * k, 2),
            Greeting => range(base + ch::CARD_GREETING, 2 * ch::GREETING_UNITS),
            StyleUse(k) => range(base + ch::STYLE_USE + 2 * k, 2),
            Slot(k) => [range(s.base(k), slots::LEN), range(save::SLOT_USED + k, 1), range(save::LAST_PLAYED, 1)].concat(),
            MySet(k, p) => {
                let o = base + sets::MY_SETS + sets::MY_SET * k;
                match p {
                    SetPart::Name => range(o, sets::NAME_LEN),
                    // the seven box indices and the decorations copied with them
                    SetPart::Gear => range(o + sets::GEAR, sets::PIGMENT - sets::GEAR),
                    // colours, colour modes and own-colour flags
                    SetPart::Pigment => range(o + sets::PIGMENT, sets::STYLE - sets::PIGMENT),
                    // style, arts, SP bits
                    SetPart::Arts => range(o + sets::STYLE, sets::SP + 1 - sets::STYLE),
                }
            }
            PalicoSet(k) => range(base + sets::PALICO_SETS + sets::PALICO_SET * k, sets::PALICO_SET),
            Pigment => {
                let colours = ch::LOOK_COLOURS.iter().flat_map(|&c| range(base + c, 20));
                [sets::PLAYER_OWN, sets::HDR_OWN].iter().flat_map(|&a| range(base + a, 2)).chain([sets::PLAYER_MODES, sets::HDR_MODES].iter().flat_map(|&a| range(base + a, 4))).chain(colours).collect()
            }
            Arts => [
                range(base + sets::PLAYER_ARTS, 8),
                range(base + sets::HDR_ARTS, 8),
                ch::LOOKS[..2].iter().map(|&l| (base + l + sets::LOOK_STYLE, 0xFF)).collect(),
            ]
            .concat(),
            Unlock(m) => {
                let mp = &unlocks::maps()[m];
                let mut f = range(base + mp.on, mp.bytes);
                if let Some(n) = mp.new {
                    f.extend(range(base + n, mp.bytes));
                }
                match mp.id.as_str() {
                    "lab" => {
                        for a in [unlocks::LAB_OFFERED, unlocks::LAB_OFFERED_NEW] {
                            f.extend(range(base + a, mp.bytes));
                        }
                        // the supply sets that follow the Lab, never set 2 (the Division's)
                        for k in (1..=24).filter(|&k| unlocks::supply_lab(k).is_some()) {
                            f.extend([bit(base, unlocks::SUPPLY, k), bit(base, unlocks::SUPPLY_NEW, k)]);
                        }
                    }
                    "notes2" => f.extend(unlocks::entries(m).iter().map(|e| bit(base, monsters::NOTES_NEW, e.id as usize))),
                    _ => {}
                }
                f
            }
            // the adoption's default costume and event flags; 769 is every pet's (restore)
            Pet(k) => {
                let c = unlocks::PET_DEFAULT[k] as usize;
                let mut f = [range(base + unlocks::PET_NAMES + unlocks::PET_NAME_LEN * k, unlocks::PET_NAME_LEN), range(base + unlocks::PET_COSTUMES + k, 1)].concat();
                f.extend([bit(base, unlocks::PETS_ADOPTED, k), bit(base, unlocks::COSTUMES, c), bit(base, unlocks::COSTUMES_NEW, c), bit(base, pg::FLAGS, unlocks::ADOPT_FLAG)]);
                if k > 0 {
                    f.push(bit(base, pg::FLAGS, unlocks::POOGIE_FLAGS + k));
                }
                f
            }
            Moofahs => {
                let (on, new, _) = gc::Map::Words.at();
                [range(base + unlocks::MOOFAHS, unlocks::PETS), vec![bit(base, on, unlocks::MOOFAH_WORD), bit(base, new, unlocks::MOOFAH_WORD)]].concat()
            }
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
            // another pet still adopted keeps the flag every adoption raises
            Target::Pet(_) if (0..unlocks::PETS).any(|k| unlocks::adopted(live, base, k)) => {
                Char::new(&mut *live, slot).set_flag(unlocks::ADOPT_FLAG, true);
            }
            _ => {}
        }
    }
}
