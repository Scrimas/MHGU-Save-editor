//! The item box and the equipment boxes as CSV files, and talismans as text.
//!
//! Item box: `slot,id,name,count`, one used slot a line (slot 1-based). The name is for
//! people: a line without an ID is looked up by name, a line without a slot takes the
//! next one after the line before.
//!
//! Equipment box: `slot,type,id,level,name,raw`. `raw` holds the whole 36-byte entry in
//! hex (decorations, transmog, talisman skills); type, ID and level are applied over it,
//! so they can be edited in a spreadsheet; a line without `raw` makes a fresh entry.
//!
//! Talismans: the charm list of Athena's Armor Set Search (`mycharms.txt`):
//! `Slots,Skill1,Points1,Skill2,Points2` with the game's English skill names.

use crate::equipment::{self, Entry, Kind, Owner, Talisman, ENTRY};
use crate::items::{self, Stack};
use crate::save::Save;
use crate::{palico, sets};
use std::collections::BTreeSet;

/// What is wrong with a line of an imported file (1-based line number).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The file is not CSV of this kind (its header, or a malformed line).
    Format,
    /// A slot number outside the box.
    Slot(String),
    /// Two lines for one slot.
    Twice(usize),
    /// An item or equipment ID, or a name nothing is called.
    Id(String),
    Count(String),
    Level(String),
    Raw,
    /// More lines than the box has slots.
    Full,
    /// A talisman skill nothing is called.
    Skill(String),
    Points(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineError {
    pub line: usize,
    pub problem: Problem,
}

fn err(line: usize, problem: Problem) -> LineError {
    LineError { line, problem }
}

fn reader(text: &str) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new().has_headers(true).flexible(true).trim(csv::Trim::All).comment(Some(b'#')).from_reader(text.as_bytes())
}

/// The column of `name` in the header, if there.
fn column(h: &csv::StringRecord, name: &str) -> Option<usize> {
    h.iter().position(|c| c.eq_ignore_ascii_case(name))
}

fn writer() -> csv::Writer<Vec<u8>> {
    csv::WriterBuilder::new().terminator(csv::Terminator::CRLF).from_writer(vec![])
}

fn finish(w: csv::Writer<Vec<u8>>) -> String {
    String::from_utf8(w.into_inner().expect("writes to memory")).expect("UTF-8 fields")
}

/// Where each line of a box file goes: its slot, or the one after the line before.
struct Slots {
    len: usize,
    next: usize,
    taken: BTreeSet<usize>,
}

impl Slots {
    fn take(&mut self, line: usize, field: Option<&str>) -> Result<usize, LineError> {
        let i = match field.filter(|f| !f.is_empty()) {
            Some(f) => match f.parse::<usize>() {
                Ok(n) if (1..=self.len).contains(&n) => n - 1,
                _ => return Err(err(line, Problem::Slot(f.into()))),
            },
            None => (self.next..self.len).find(|i| !self.taken.contains(i)).ok_or(err(line, Problem::Full))?,
        };
        if !self.taken.insert(i) {
            return Err(err(line, Problem::Twice(i + 1)));
        }
        self.next = i + 1;
        Ok(i)
    }
}

/// The item box (or any slot list) as CSV.
pub fn items_csv(v: &[Stack], name: impl Fn(u16) -> String) -> String {
    let mut w = writer();
    w.write_record(["slot", "id", "name", "count"]).unwrap();
    for (i, st) in v.iter().enumerate().filter(|(_, s)| !s.is_empty()) {
        w.write_record([(i + 1).to_string(), st.id.to_string(), name(st.id), st.count.to_string()]).unwrap();
    }
    finish(w)
}

/// `len` slots from a file in `items_csv`'s format; slots it leaves out are empty.
/// `lookup` finds an item ID by name, for lines without one.
pub fn read_items(text: &str, len: usize, lookup: impl Fn(&str) -> Option<u16>) -> Result<Vec<Stack>, LineError> {
    let mut r = reader(text);
    let h = r.headers().map_err(|_| err(1, Problem::Format))?.clone();
    let (cs, ci, cn, cc) = (column(&h, "slot"), column(&h, "id"), column(&h, "name"), column(&h, "count"));
    let Some(cc) = cc.filter(|_| ci.is_some() || cn.is_some()) else { return Err(err(1, Problem::Format)) };
    let mut out = vec![Stack::default(); len];
    let mut slots = Slots { len, next: 0, taken: BTreeSet::new() };
    for rec in r.records() {
        let rec = rec.map_err(|e| err(e.position().map_or(0, |p| p.line() as usize), Problem::Format))?;
        let line = rec.position().map_or(0, |p| p.line() as usize);
        let f = |c: Option<usize>| c.and_then(|c| rec.get(c)).unwrap_or("");
        let id = match (f(ci), f(cn)) {
            ("", "") => return Err(err(line, Problem::Id(String::new()))),
            ("", n) => lookup(n).ok_or(err(line, Problem::Id(n.into())))?,
            (i, _) => i.parse::<u16>().ok().filter(|&i| (1..=items::MAX_ID).contains(&i)).ok_or(err(line, Problem::Id(i.into())))?,
        };
        let count = f(Some(cc));
        let count = count.parse::<u8>().ok().filter(|c| (1..=items::MAX_COUNT).contains(c)).ok_or(err(line, Problem::Count(count.into())))?;
        let i = slots.take(line, cs.and_then(|c| rec.get(c)))?;
        out[i] = Stack { id, count };
    }
    Ok(out)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<[u8; ENTRY]> {
    if s.len() != 2 * ENTRY || !s.is_ascii() {
        return None;
    }
    let v: Option<Vec<u8>> = (0..ENTRY).map(|k| u8::from_str_radix(&s[2 * k..2 * k + 2], 16).ok()).collect();
    v?.try_into().ok()
}

/// An equipment box as CSV; empty entries left out.
pub fn equipment_csv(v: &[Entry], name: impl Fn(&Entry) -> String) -> String {
    let mut w = writer();
    w.write_record(["slot", "type", "id", "level", "name", "raw"]).unwrap();
    for (i, e) in v.iter().enumerate().filter(|(_, e)| !e.is_empty()) {
        w.write_record([(i + 1).to_string(), e.kind().code().to_string(), e.id().to_string(), e.level().to_string(), name(e), hex(&e.raw)]).unwrap();
    }
    finish(w)
}

/// `len` entries from a file in `equipment_csv`'s format; slots it leaves out are empty.
pub fn read_equipment(text: &str, len: usize) -> Result<Vec<Entry>, LineError> {
    let mut r = reader(text);
    let h = r.headers().map_err(|_| err(1, Problem::Format))?.clone();
    let (cs, ct, ci, cl, cr) = (column(&h, "slot"), column(&h, "type"), column(&h, "id"), column(&h, "level"), column(&h, "raw"));
    if cr.is_none() && (ct.is_none() || ci.is_none()) {
        return Err(err(1, Problem::Format));
    }
    let empty = Entry { raw: [0; ENTRY] };
    let mut out = vec![empty; len];
    let mut slots = Slots { len, next: 0, taken: BTreeSet::new() };
    for rec in r.records() {
        let rec = rec.map_err(|e| err(e.position().map_or(0, |p| p.line() as usize), Problem::Format))?;
        let line = rec.position().map_or(0, |p| p.line() as usize);
        let f = |c: Option<usize>| c.and_then(|c| rec.get(c)).unwrap_or("");
        let mut e = match f(cr) {
            "" => None,
            x => Some(Entry { raw: unhex(x).ok_or(err(line, Problem::Raw))? }),
        };
        let kind = match f(ct) {
            "" => None,
            t => Some(t.parse::<u8>().ok().filter(|&c| (1..32).contains(&c)).map(Kind::from_code).ok_or(err(line, Problem::Id(t.into())))?),
        };
        let id = match f(ci) {
            "" => None,
            i => Some(i.parse::<u16>().map_err(|_| err(line, Problem::Id(i.into())))?),
        };
        // type and ID over the raw entry; a fresh one without it
        match (&mut e, kind, id) {
            (Some(x), k, i) => {
                if let Some(k) = k.filter(|&k| k != x.kind()) {
                    let w = u16::from_le_bytes([x.raw[0], x.raw[1]]) & !0x1F | k.code() as u16;
                    x.raw[..2].copy_from_slice(&w.to_le_bytes());
                }
                if let Some(i) = i {
                    x.raw[2..4].copy_from_slice(&i.to_le_bytes());
                }
            }
            (None, Some(k), Some(i)) => e = Some(Entry::new(k, i)),
            _ => return Err(err(line, Problem::Format)),
        }
        let mut e = e.unwrap();
        match f(cl) {
            "" => {}
            l => e.set_level(l.parse::<u8>().ok().filter(|l| (1..=32).contains(l)).ok_or(err(line, Problem::Level(l.into())))?),
        }
        let i = slots.take(line, cs.and_then(|c| rec.get(c)))?;
        out[i] = e;
    }
    Ok(out)
}

/// Box entries something refers to by index, which an import keeps: the worn gear and
/// the My Sets (hunter), the Palicoes' gear and the Palico sets (Palico).
pub fn referenced(s: &Save, base: usize, owner: Owner) -> BTreeSet<usize> {
    let mut v = BTreeSet::new();
    match owner {
        Owner::Hunter => {
            v.extend((0..sets::PIECES).map(|p| s.u16(base + equipment::WORN + 2 * p) as usize));
            for k in 0..sets::MY_SETS_N {
                v.extend(sets::my_set(s, base, k).gear.iter().map(|&g| g as usize));
            }
        }
        Owner::Palico => {
            for i in (0..palico::PLACES).filter(|&i| !palico::get(s, base, i).name.is_empty()) {
                v.extend(palico::gear(s, base, i));
            }
            for k in 0..sets::PALICO_SETS_N {
                v.extend(sets::palico_set(s, base, k).gear.iter().map(|&g| g as usize));
            }
        }
    }
    v.retain(|&i| i < owner.len());
    v
}

// --- talismans ------------------------------------------------------------------------

/// A talisman as a line of Athena's charm list.
pub fn talisman_line(t: &Talisman, skill: impl Fn(u8) -> String) -> String {
    let part = |k: usize| if t.skills[k] == 0 { ",".to_string() } else { format!("{},{}", skill(t.skills[k]), t.points[k]) };
    format!("{},{},{}", t.slots, part(0), part(1))
}

pub const TALISMAN_HEADER: &str = "#Format: Slots,Skill1,Points1,Skill2,Points2";

/// Talismans from charm-list text: one a line, `#` lines and blank ones skipped. The
/// tier is left 0 for the caller to pick. `lookup` finds a skill ID by name.
pub fn read_talismans(text: &str, lookup: impl Fn(&str) -> Option<u8>) -> Result<Vec<Talisman>, LineError> {
    let mut out = vec![];
    for (n, l) in text.lines().enumerate() {
        let line = n + 1;
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if !(3..=5).contains(&f.len()) {
            return Err(err(line, Problem::Format));
        }
        let slots = f[0].parse::<u8>().ok().filter(|&s| s <= 3).ok_or(err(line, Problem::Slot(f[0].into())))?;
        let mut t = Talisman { skills: [0; 2], points: [0; 2], slots, tier: 0 };
        for k in 0..2 {
            let (name, pts) = (f.get(1 + 2 * k).copied().unwrap_or(""), f.get(2 + 2 * k).copied().unwrap_or(""));
            if name.is_empty() && pts.is_empty() {
                continue;
            }
            t.skills[k] = lookup(name).filter(|&s| s != 0).ok_or(err(line, Problem::Skill(name.into())))?;
            t.points[k] = pts.parse::<i8>().ok().filter(|&p| p != 0).ok_or(err(line, Problem::Points(pts.into())))?;
        }
        if t.skills[0] == 0 {
            return Err(err(line, Problem::Skill(String::new())));
        }
        out.push(t);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE as B};

    #[test]
    fn item_box_round_trip_and_hand_written_lines() {
        let v = [Stack { id: 1, count: 10 }, Stack::default(), Stack { id: 2, count: 99 }];
        let names = |id: u16| ["", "Potion", "Mega, Potion"][id as usize].to_string();
        let text = items_csv(&v, names);
        assert_eq!(text, "slot,id,name,count\r\n1,1,Potion,10\r\n3,2,\"Mega, Potion\",99\r\n");
        assert_eq!(read_items(&text, 3, |_| None).unwrap(), v.to_vec());
        // no slot: after the line before; no ID: by name
        let hand = "name,count\nPotion,5\n\"Mega, Potion\",3\n";
        let look = |n: &str| ["Potion", "Mega, Potion"].iter().position(|x| *x == n).map(|i| i as u16 + 1);
        assert_eq!(read_items(hand, 3, look).unwrap(), vec![Stack { id: 1, count: 5 }, Stack { id: 2, count: 3 }, Stack::default()]);
        assert_eq!(read_items("slot,id,count\n4,1,1\n", 3, |_| None).unwrap_err(), err(2, Problem::Slot("4".into())));
        assert_eq!(read_items("slot,id,count\n1,1,1\n1,2,1\n", 3, |_| None).unwrap_err(), err(3, Problem::Twice(1)));
        assert_eq!(read_items("id,count\n1,100\n", 3, |_| None).unwrap_err(), err(2, Problem::Count("100".into())));
        assert_eq!(read_items("id,count\n1,1\n1,1\n", 1, |_| None).unwrap_err(), err(3, Problem::Full));
        assert_eq!(read_items("nothing\n", 1, |_| None).unwrap_err(), err(1, Problem::Format));
    }

    #[test]
    fn equipment_box_round_trip_and_edits() {
        let mut a = Entry::new(Kind::Head, 321);
        a.set_level(7);
        a.add_deco(2700);
        let t = Entry::new(Kind::Talisman, 7);
        let v = vec![a.clone(), Entry { raw: [0; ENTRY] }, t.clone()];
        let text = equipment_csv(&v, |e| format!("#{}", e.id()));
        assert_eq!(read_equipment(&text, 3).unwrap(), v);
        // level and ID edited over raw; a line without raw makes a fresh entry
        let edited = text.replace(",7,#321,", ",9,#321,").replace("1,1,321,", "1,1,322,");
        let r = read_equipment(&edited, 4).unwrap();
        assert_eq!((r[0].id(), r[0].level(), r[0].decos()[0]), (322, 9, 2700));
        let fresh = read_equipment("type,id,level\n7,15,3\n", 2).unwrap();
        assert_eq!((fresh[0].kind(), fresh[0].id(), fresh[0].level()), (Kind::Weapon(0), 15, 3));
        assert_eq!(read_equipment("slot,raw\n1,00\n", 2).unwrap_err(), err(2, Problem::Raw));
    }

    #[test]
    fn referenced_entries_are_the_worn_ones_and_sets() {
        let mut s = blank();
        for k in 0..sets::MY_SETS_N {
            sets::clear_my_set(&mut s, B, k);
        }
        for p in 0..sets::PIECES {
            s.set_u16(B + equipment::WORN + 2 * p, 0xFFFF);
        }
        s.set_u16(B + equipment::WORN, 5);
        sets::set_my_set_piece(&mut s, B, 3, 1, Some(9));
        let r = referenced(&s, B, Owner::Hunter);
        assert!(r.contains(&5) && r.contains(&9) && !r.contains(&0));
    }

    #[test]
    fn talismans_as_athena_lines() {
        let t = Talisman { skills: [18, 69], points: [5, -3], slots: 2, tier: 99 };
        let name = |s: u8| if s == 18 { "Attack".to_string() } else { "Expert".to_string() };
        assert_eq!(talisman_line(&t, name), "2,Attack,5,Expert,-3");
        let one = Talisman { skills: [18, 0], points: [7, 0], slots: 0, tier: 99 };
        assert_eq!(talisman_line(&one, name), "0,Attack,7,,");
        let look = |n: &str| match n.to_lowercase().as_str() {
            "attack" => Some(18),
            "expert" => Some(69),
            _ => None,
        };
        let text = format!("{TALISMAN_HEADER}\n2,Attack,5,Expert,-3\n\n0,attack,7,,\n1,Attack,4\n");
        let r = read_talismans(&text, look).unwrap();
        assert_eq!(r.len(), 3);
        assert_eq!((r[0].skills, r[0].points, r[0].slots), ([18, 69], [5, -3], 2));
        assert_eq!((r[1].skills, r[2].skills), ([18, 0], [18, 0]));
        assert_eq!(read_talismans("1,Nope,3,,", look).unwrap_err(), err(1, Problem::Skill("Nope".into())));
        assert_eq!(read_talismans("4,Attack,3,,", look).unwrap_err(), err(1, Problem::Slot("4".into())));
        assert_eq!(read_talismans("1,Attack,0,,", look).unwrap_err(), err(1, Problem::Points("0".into())));
    }
}
