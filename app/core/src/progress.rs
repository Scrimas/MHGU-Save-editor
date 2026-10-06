//! Quests, unlock rules, quest sets, villager requests, star levels and the collection
//! bitfields. Ports tools/quest_unlock.py, tools/request_offer.py and
//! tools/complete_quests.py; layouts in docs/05, 08, 09, 10.

use crate::data::{tables, Quest};
use crate::save::Save;
use std::ops::{Deref, DerefMut};

pub const HR: usize = 0x28;
pub const CLEARED: usize = 0x2C77;
pub const SEEN: usize = 0x2D77;
pub const FAILED: usize = 0x2E77;
pub const PROGRESS: usize = 0x2F77;
pub const QUESTSETS: usize = 0x3187;
pub const VIL_STAR: usize = 0x2C4DA;
pub const HUB_STAR: usize = 0x2C4DC;
pub const FLAGS: usize = 0x2C56D;
pub const NPC_HOLD: usize = 0x2C62D;
pub const ROTATION: usize = 0x504B;
pub const POINTS: usize = 0x281B;
pub const ARTS: usize = 0x2C13;
pub const INGREDIENTS: usize = 0x2F8F;
pub const DISHES: usize = 0x2C67D;
pub const AWARDS_GAME: usize = 0x3157;
pub const AWARDS_CARD: usize = 0xC8115;
pub const PERMITS: usize = 0x283C;
/// Quest sets that need Arena records and are never set by the bulk completion.
pub const RANK_SETS: [u32; 5] = [46, 47, 53, 78, 79];
pub const VILLAGES: [&str; 4] = ["Bherna", "Kokoto", "Pokke", "Yukumo"];
pub const DEVIANTS: [&str; 18] = [
    "Redhelm Arzuros", "Snowbaron Lagombi", "Stonefist Hermitaur", "Dreadqueen Rathian", "Drilltusk Tetsucabra",
    "Silverwind Nargacuga", "Crystalbeard Uragaan", "Deadeye Yian Garuga", "Dreadking Rathalos", "Thunderlord Zinogre",
    "Grimclaw Tigrex", "Hellblade Glavenus", "Nightcloak Malfestio", "Rustrazor Ceanataur", "Soulseer Mizutsune",
    "Boltreaver Astalos", "Elderfrost Gammoth", "Bloodbath Diablos",
];
/// First quest index of the deviant level bits (Special Permit quests 947-1174).
pub const DEVIANT_QUEST0: usize = 947;
/// The G-rank quests (IDs) that release a deviant's G1, any one cleared (docs/03 "Unlock
/// gating", from the unlock script; the gate itself is CONFIRMED in game). Bloodbath is
/// released by event flag `BLOODBATH_FLAG` instead.
pub const DEVIANT_GATE: [&[u32]; 18] = [
    &[11125, 11104],
    &[11108],
    &[11113, 11111],
    &[11248, 11210, 11226, 11352, 11306],
    &[11128, 11110],
    &[11216, 11234],
    &[11303],
    &[11302, 11347],
    &[11352, 11306, 11357, 11359],
    &[11308, 11355],
    &[11405, 11462],
    &[11467],
    &[11206, 11235, 11348],
    &[11214, 11250],
    &[11310, 11356],
    &[11311, 11358],
    &[11312, 11354],
    &[],
];
pub const BLOODBATH_FLAG: usize = 1226;

/// Access to one character: read through `&Save`, read and write through `&mut Save`
/// (the setters need the latter).
pub struct Char<S> {
    pub s: S,
    pub base: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestBit {
    Cleared,
    Seen,
    Failed,
}

impl QuestBit {
    fn off(self) -> usize {
        match self {
            QuestBit::Cleared => CLEARED,
            QuestBit::Seen => SEEN,
            QuestBit::Failed => FAILED,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Lock {
    Unlocked,
    /// The rule holds but the quest is a rotating one whose bit is clear.
    Rotated,
    /// Event quest: listed once downloaded.
    Event,
    /// Alternatives, each a list of what is missing.
    Locked(Vec<Vec<String>>),
}

impl<S: Deref<Target = Save>> Char<S> {
    pub fn new(s: S, slot: usize) -> Self {
        let base = s.base(slot);
        Char { s, base }
    }
    pub fn bit(&self, off: usize, i: usize) -> bool {
        self.s.bit(self.base + off, i)
    }
    pub fn set_bit(&mut self, off: usize, i: usize, v: bool)
    where
        S: DerefMut,
    {
        self.s.set_bit(self.base + off, i, v)
    }
    pub fn u16(&self, off: usize) -> u16 {
        self.s.u16(self.base + off)
    }
    pub fn u32(&self, off: usize) -> u32 {
        self.s.u32(self.base + off)
    }

    // --- quests -------------------------------------------------------------------

    pub fn quest(&self, which: QuestBit, index: usize) -> bool {
        self.bit(which.off(), index)
    }
    pub fn set_quest(&mut self, which: QuestBit, index: usize, v: bool)
    where
        S: DerefMut,
    {
        self.set_bit(which.off(), index, v)
    }
    fn cleared_id(&self, id: u32) -> bool {
        tables().quest_index(id).is_some_and(|i| self.quest(QuestBit::Cleared, i))
    }
    /// HR as the game computes it on load (the slot header copy is only a summary).
    pub fn hr(&self) -> u16 {
        crate::character::get(&*self.s, self.base).hr
    }
    pub fn village_star(&self) -> u16 {
        self.u16(VIL_STAR)
    }
    pub fn hub_star(&self) -> u16 {
        self.u16(HUB_STAR)
    }
    pub fn set_village_star(&mut self, v: u16)
    where
        S: DerefMut,
    {
        self.s.set_u16(self.base + VIL_STAR, v.clamp(1, 10))
    }
    /// Also refreshes the HR copies, which follow the Hub star below HR 13.
    pub fn set_hub_star(&mut self, v: u16)
    where
        S: DerefMut,
    {
        crate::character::set_hub_star(&mut *self.s, self.base, v)
    }

    /// The unlock rule of `script\check_quest_unlocked` for this quest (quest-unlock.csv).
    pub fn lock(&self, id: u32) -> Lock {
        let t = tables();
        let rule = match t.unlock.get(&id) {
            None => return Lock::Event,
            Some(r) if r.is_empty() => return Lock::Event,
            Some(r) => r,
        };
        let needs: Vec<Vec<String>> =
            rule.split(" OR ").map(|alt| alt.split(" AND ").filter_map(|p| self.quest_missing(p)).collect()).collect();
        if needs.iter().any(Vec::is_empty) {
            match t.rotating.get(&id) {
                Some(&b) if !self.bit(ROTATION, b) => Lock::Rotated,
                _ => Lock::Unlocked,
            }
        } else {
            Lock::Locked(needs)
        }
    }

    fn quest_missing(&self, pred: &str) -> Option<String> {
        let (kind, arg) = pred.split_once(':').unwrap_or((pred, ""));
        let n = || arg.parse::<u32>().unwrap_or(0);
        match kind {
            "always" => None,
            "flag" => (!self.bit(FLAGS, n() as usize)).then(|| format!("event flag {arg}")),
            "cleared" => (!self.cleared_id(n())).then(|| format!("clear quest {arg}")),
            "hr" => (self.hr() < n() as u16).then(|| format!("HR {arg}")),
            "hub_star" => (self.hub_star() < n() as u16).then(|| format!("Hub ★{arg}")),
            "all" => {
                let todo: Vec<&str> = arg.split('|').filter(|q| !self.cleared_id(q.parse().unwrap_or(0))).collect();
                (!todo.is_empty()).then(|| format!("clear all of {}", todo.join(" ")))
            }
            "atleast" => {
                let (need, qs) = arg.split_once(':').unwrap_or(("0", ""));
                let qs: Vec<&str> = qs.split('|').collect();
                let have = qs.iter().filter(|q| self.cleared_id(q.parse().unwrap_or(0))).count();
                let need: usize = need.parse().unwrap_or(0);
                (have < need).then(|| format!("clear {} more of {}", need - have, qs.join(" ")))
            }
            _ => Some(format!("? {pred}")),
        }
    }

    pub fn set_complete(&self, set: u32) -> bool {
        let m = Char::set_members(set);
        !m.is_empty() && m.iter().all(|&i| self.quest(QuestBit::Cleared, i))
    }

    /// Clear (and see) quests, then derive what a real clear leaves behind: the quest
    /// set bits of every set that is now complete (not the Arena rank sets). Returns the
    /// sets that were set. Star levels are raised only if `stars` (the game raises them
    /// when all urgents of a level are cleared; see docs/05).
    pub fn clear_quests(&mut self, indices: &[usize]) -> Vec<u32>
    where
        S: DerefMut,
    {
        for &i in indices {
            self.set_quest(QuestBit::Cleared, i, true);
            self.set_quest(QuestBit::Seen, i, true);
        }
        let mut sets: Vec<u32> = tables().quests.iter().flat_map(|q| q.sets.iter().copied()).collect();
        sets.sort_unstable();
        sets.dedup();
        let mut done = vec![];
        for s in sets {
            if !RANK_SETS.contains(&s) && !self.bit(QUESTSETS, s as usize) && self.set_complete(s) {
                self.set_bit(QUESTSETS, s as usize, true);
                done.push(s);
            }
        }
        done
    }

    // --- villager requests --------------------------------------------------------

    pub fn flag(&self, f: usize) -> bool {
        self.bit(FLAGS, f)
    }
    pub fn set_flag(&mut self, f: usize, v: bool)
    where
        S: DerefMut,
    {
        self.set_bit(FLAGS, f, v)
    }

    /// Accept a request the way its NPC does: the accepted flag plus the other flags of
    /// its first offer block (request-offer.csv `also`).
    pub fn accept_request(&mut self, index: usize)
    where
        S: DerefMut,
    {
        let t = tables();
        if let Some(o) = t.offers.iter().find(|o| o.index == index) {
            self.set_flag(o.accept_flag, true);
            for &f in &o.also {
                self.set_flag(f.unsigned_abs() as usize, f > 0);
            }
        } else if let Some(f) = t.requests.iter().find(|r| r.index == index).and_then(|r| r.accept_flag) {
            self.set_flag(f, true);
        }
    }

    /// What the NPC still waits for before offering request `index` (empty = ready).
    pub fn offer_missing(&self, index: usize) -> Vec<String> {
        let t = tables();
        let Some(o) = t.offers.iter().find(|o| o.index == index) else { return vec![] };
        let mut miss = vec![];
        for group in o.offer.split(" AND ").filter(|g| !g.is_empty()) {
            let ms: Vec<Option<String>> = group.split('|').map(|c| self.offer_cond(c)).collect();
            if ms.iter().all(Option::is_some) {
                miss.push(ms.into_iter().flatten().collect::<Vec<_>>().join(" or "));
            }
        }
        miss
    }

    fn offer_cond(&self, c: &str) -> Option<String> {
        let (kind, arg) = c.split_once(':').unwrap_or((c, ""));
        let n = || arg.parse::<i64>().unwrap_or(0);
        let vil = self.village_star() as i64;
        let hub = self.hub_star() as i64;
        let t = tables();
        match kind {
            "flag" => (!self.flag(n() as usize)).then(|| format!("event flag {arg}")),
            "notflag" => self.flag(n() as usize).then(|| format!("event flag {arg} clear")),
            "cleared" => (!self.cleared_id(n() as u32)).then(|| format!("clear quest {arg}")),
            "notcleared" => self.cleared_id(n() as u32).then(|| format!("quest {arg} not cleared")),
            "village_star" => (vil < n()).then(|| format!("Village ★{arg}")),
            "hub_star" => (hub < n()).then(|| format!("Hub ★{arg}")),
            "village_eq" => (vil != n()).then(|| format!("Village ★ exactly {arg}")),
            "hub_eq" => (hub != n()).then(|| format!("Hub ★ exactly {arg}")),
            "village_max" => (vil > n()).then(|| format!("Village ★ at most {arg}")),
            "hub_max" => (hub > n()).then(|| format!("Hub ★ at most {arg}")),
            "hr" => ((self.hr() as i64) < n()).then(|| format!("HR {arg}")),
            "hr_unlocked" => (!self.bit(PROGRESS, 20)).then(|| "HR limit released".into()),
            "listed" if arg == "10646" && self.bit(PROGRESS, 31) => None,
            "listed" => match t.quest_index(n() as u32).map(|_| self.lock(n() as u32)) {
                Some(Lock::Unlocked) => None,
                _ => Some(format!("quest {arg} listed on the board")),
            },
            "questset" => (!self.bit(QUESTSETS, n() as usize)).then(|| format!("quest set {arg}")),
            "npc_idle" => {
                let b = t.npc_bit.get(arg)?;
                (0..3).any(|m| self.bit(NPC_HOLD + 24 * m, *b)).then(|| "NPC on hold until the next quest".into())
            }
            "footbath" | "not_footbath" => None,
            "points" => {
                let (name, need) = arg.split_once(':')?;
                let v = VILLAGES.iter().position(|x| *x == name)?;
                let have = self.u32(POINTS + 4 * v).saturating_add(self.u32(POINTS + 16 + 4 * v)).min(20000);
                (have < need.parse().unwrap_or(0)).then(|| format!("{name} points {have}/{need}"))
            }
            "requests_done" => {
                let (rng, need) = arg.split_once(':')?;
                let (lo, hi) = rng.split_once('-')?;
                let (lo, hi): (usize, usize) = (lo.parse().ok()?, hi.parse().ok()?);
                let have = t.requests.iter().filter(|r| (lo..=hi).contains(&r.index)).filter(|r| r.done_flag.is_some_and(|f| self.flag(f))).count();
                (have < need.parse().unwrap_or(0)).then(|| format!("{need} of requests {rng} completed (now {have})"))
            }
            "village_keys" | "group_done" => {
                let g = arg.parse::<u32>().ok()? + if kind == "group_done" { 10 } else { 0 };
                let mut alts: std::collections::BTreeMap<i64, Vec<usize>> = Default::default();
                for q in t.quests.iter().filter(|q| q.group == g) {
                    alts.entry(if q.alt != 0 { q.alt as i64 } else { -(q.index as i64) }).or_default().push(q.index);
                }
                let ok = alts.values().all(|a| a.iter().any(|&i| self.quest(QuestBit::Cleared, i)));
                (!ok).then(|| format!("all key quests of group {g}"))
            }
            _ => Some(format!("? {c}")),
        }
    }

    // --- collections --------------------------------------------------------------

    pub fn art(&self, id: u32) -> bool {
        self.bit(ARTS, id as usize)
    }
    pub fn set_art(&mut self, id: u32, v: bool)
    where
        S: DerefMut,
    {
        assert!((1..=70).contains(&id) || (83..=190).contains(&id), "not a Hunter Art ID");
        self.set_bit(ARTS, id as usize, v)
    }
    pub fn ingredient(&self, b: usize) -> bool {
        self.bit(INGREDIENTS, b)
    }
    pub fn set_ingredient(&mut self, b: usize, v: bool)
    where
        S: DerefMut,
    {
        assert!(b < 45);
        self.set_bit(INGREDIENTS, b, v)
    }
    pub fn dish(&self, b: usize) -> bool {
        self.bit(DISHES, b)
    }
    pub fn set_dish(&mut self, b: usize, v: bool)
    where
        S: DerefMut,
    {
        assert!(b < 99);
        self.set_bit(DISHES, b, v)
    }
    /// The Guild Card award bit (what the card shows).
    pub fn award(&self, b: usize) -> bool {
        self.bit(AWARDS_CARD, b)
    }
    /// Awards live in two maps: the card ORs the game-side map back in after every
    /// quest, so a cleared award must be cleared in both (docs/09).
    pub fn set_award(&mut self, b: usize, v: bool)
    where
        S: DerefMut,
    {
        assert!(b < 132);
        self.set_bit(AWARDS_CARD, b, v);
        self.set_bit(AWARDS_GAME, b, v);
    }

    // --- deviants -----------------------------------------------------------------

    pub fn permits(&self, d: usize) -> u8 {
        self.s.u8(self.base + PERMITS + d)
    }
    pub fn set_permits(&mut self, d: usize, v: u8)
    where
        S: DerefMut,
    {
        assert!(d < 18);
        self.s.set_u8(self.base + PERMITS + d, v.min(99))
    }
    /// Whether the board offers G1 and up: the base monster's G-rank gate is open.
    pub fn deviant_gate_open(&self, d: usize) -> bool {
        if d == 17 {
            return self.flag(BLOODBATH_FLAG);
        }
        DEVIANT_GATE[d].iter().any(|&id| self.cleared_id(id))
    }
}

/// What depends on the tables only (called as `Char::real_quests` and so on).
impl Char<&Save> {
    /// Quest sets whose members are all cleared (first occurrence of a repeated ID).
    pub fn set_members(set: u32) -> Vec<usize> {
        let t = tables();
        let mut seen = std::collections::HashSet::new();
        t.quests
            .iter()
            .filter(|q| seen.insert(q.id))
            .filter(|q| q.sets.contains(&set))
            .map(|q| q.index)
            .collect()
    }

    /// Real quests of the board (placeholders, unused event slots and repeated IDs skipped).
    pub fn real_quests(events: bool) -> Vec<&'static Quest> {
        let mut seen = std::collections::HashSet::new();
        tables()
            .quests
            .iter()
            .filter(|q| q.is_real() && seen.insert(q.id))
            .filter(|q| events || !q.category.starts_with("Event"))
            .collect()
    }

    /// (first quest index, number of levels) of deviant `d`: 16 for the first 12, 6 after.
    pub fn deviant_levels(d: usize) -> (usize, usize) {
        if d < 12 { (DEVIANT_QUEST0 + 16 * d, 16) } else { (DEVIANT_QUEST0 + 192 + 6 * (d - 12), 6) }
    }

    /// Level number of G1 among `deviant_levels` (after Lv1-10, or the first).
    pub fn deviant_g1(d: usize) -> usize {
        if d < 12 { 10 } else { 0 }
    }
}
