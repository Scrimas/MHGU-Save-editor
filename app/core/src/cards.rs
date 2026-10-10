//! Other hunters' data: the stored Guild Cards (list 1), the Guild Card inbox (list 2), the
//! blocked-user list and the Hunters for Hire (docs/11-save-map.md, "Guild Card manager",
//! the sBlackList and sGuestHunter rows). DERIVED from the game's code.
//!
//! A card list is a run of elements: u32 zlib length, a zlib stream of one 6328-byte card,
//! u32 state (3 card, 1 empty), a 36-byte trailer (u16 HR, 11 UTF-16 name, 8-byte ID, u32
//! play time). Padding follows so the list keeps its size (each element counts 6328 + 8
//! bytes); the loader skips it unread. The game keeps the cards at the front: it counts
//! the leading cards, shows only those and fills the first empty element, and its Delete
//! (`0x164e5c`, inbox `0x165624`) moves the later cards and their info records up. Nothing
//! else in the save names a card by its slot (Unity, Arena times and the Hunters for Hire
//! go by ID or keep copies).

use crate::save::Save;

/// A card, inflated.
pub const CARD_LEN: usize = 0x18B8;
const TRAILER: usize = 36;
const FILLED: u32 = 3;
const EMPTY: u32 = 1;
/// The zlib stream of 6328 zeros the game writes in an empty element.
const EMPTY_Z: [u8; 29] = [
    0x78, 0x9c, 0xed, 0xc1, 0x31, 0x01, 0x00, 0x00, 0x00, 0xc2, 0xa0, 0xf5, 0x4f, 0x6d, 0x0d, 0x0f, 0xa0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3e,
    0x0c, 0x18, 0xb8, 0x00, 0x01,
];
/// Card type of a card moved in from the inbox (*StreetPass*, `GuildCardMsg` 263).
pub const STREETPASS: i8 = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum List {
    /// The Guild Cards kept (*Card List*), 100.
    Stored,
    /// Cards received by the Courier Service (*Guild Card Inbox*), 50.
    Inbox,
}

impl List {
    /// (list, elements, info records, info record size), relative to the character base.
    pub fn at(self) -> (usize, usize, usize, usize) {
        match self {
            List::Stored => (0x2C6BD, 100, 0xC8A75, 44),
            List::Inbox => (0xC9BA5, 50, 0x117125, 36),
        }
    }

    /// Bytes of the list, padding included.
    pub fn size(self) -> usize {
        self.at().1 * (CARD_LEN + 8)
    }
}

/// One element as stored.
#[derive(Clone)]
struct Elem {
    z: Vec<u8>,
    state: u32,
    trailer: Vec<u8>,
}

impl Elem {
    fn empty() -> Elem {
        Elem { z: EMPTY_Z.to_vec(), state: EMPTY, trailer: vec![0; TRAILER] }
    }
}

/// Every element of the list with its info record; None when the lengths run past the
/// list (not a list the game wrote).
fn elems(s: &Save, base: usize, l: List) -> Option<Vec<(Elem, Vec<u8>)>> {
    let (at, n, info, info_len) = l.at();
    let end = base + at + l.size();
    let mut p = base + at;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let len = s.u32(p) as usize;
        if len > CARD_LEN || p + 8 + len + TRAILER > end {
            return None;
        }
        let e = Elem { z: s.get(p + 4, len).to_vec(), state: s.u32(p + 4 + len), trailer: s.get(p + 8 + len, TRAILER).to_vec() };
        out.push((e, s.get(base + info + info_len * i, info_len).to_vec()));
        p += 8 + len + TRAILER;
    }
    Some(out)
}

/// Write `cards` at the front of the list, empty elements after them, zero padding and
/// zeroed info records to the end.
fn write(s: &mut Save, base: usize, l: List, cards: Vec<(Elem, Vec<u8>)>) {
    let (at, n, info, info_len) = l.at();
    assert!(cards.len() <= n);
    let mut list = Vec::with_capacity(l.size());
    let mut infos = vec![0u8; n * info_len];
    for i in 0..n {
        let (e, inf) = cards.get(i).cloned().unwrap_or_else(|| (Elem::empty(), vec![0; info_len]));
        list.extend((e.z.len() as u32).to_le_bytes());
        list.extend(&e.z);
        list.extend(e.state.to_le_bytes());
        list.extend(&e.trailer);
        infos[i * info_len..(i + 1) * info_len].copy_from_slice(&inf);
    }
    assert!(list.len() <= l.size(), "cards larger than the list");
    list.resize(l.size(), 0);
    s.put(base + at, &list);
    s.put(base + info, &infos);
}

fn utf16(b: &[u8]) -> String {
    let u: Vec<u16> = b.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&u)
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// A card of a list, with what the list keeps about it.
#[derive(Clone, Debug)]
pub struct Card {
    pub list: List,
    /// Its element in the list.
    pub slot: usize,
    pub name: String,
    pub hr: u16,
    /// The card owner's ID.
    pub id: [u8; 8],
    /// Play time in seconds: a card received again replaces this one only with more.
    pub playtime: u32,
    /// The card, 6328 bytes (zeros when the stream does not inflate).
    pub data: Vec<u8>,
    /// Its info record: 44 bytes (list) or 36 (inbox).
    pub info: Vec<u8>,
}

impl Card {
    /// Title: a word of `GC_Title_1`, a linking word of `GC_Title_2` (0 = none), a word.
    pub fn title(&self) -> [u16; 3] {
        [0, 1, 2].map(|k| u16_at(&self.data, 0x854 + 2 * k))
    }

    pub fn greeting(&self) -> String {
        utf16(&self.data[0x878..0x8B0])
    }

    /// Quests completed, all kinds, as the card shows them (capped at 99,999).
    pub fn quests(&self) -> u32 {
        (0..7).map(|k| u16_at(&self.data, 0x85E + 2 * k) as u32).sum::<u32>().min(99_999)
    }

    /// The weapon worn when the card was made: (equipment type, ID).
    pub fn weapon(&self) -> (u8, u16) {
        (self.data[0x54], u16_at(&self.data, 0x56))
    }

    /// Prowler card (played as a Palico).
    pub fn prowler(&self) -> bool {
        self.data[0x8B8] & 2 != 0
    }

    /// Date received (list) or updated: day, month, year.
    pub fn date(&self) -> (u8, u8, u16) {
        (self.info[0], self.info[1], u16_at(&self.info, 2))
    }

    /// The list's comment (*Add Comment*); inbox cards have none.
    pub fn comment(&self) -> String {
        if self.list == List::Stored { utf16(&self.info[4..0x1C]) } else { String::new() }
    }

    /// Unity with the card's owner: 76 for each quest together.
    pub fn unity(&self) -> u32 {
        if self.list == List::Stored { u32_at(&self.info, 0x1C) } else { 0 }
    }

    /// Card type: 0-23 *GuildCardMsg* 211-234, 25 StreetPass; inbox cards have none.
    pub fn kind(&self) -> Option<i8> {
        (self.list == List::Stored).then(|| self.info[0x20] as i8)
    }
}

/// The cards of a list in slot order.
pub fn cards(s: &Save, base: usize, l: List) -> Vec<Card> {
    let Some(v) = elems(s, base, l) else { return vec![] };
    v.into_iter()
        .enumerate()
        .filter(|(_, (e, _))| e.state == FILLED)
        .map(|(slot, (e, info))| {
            let mut data = miniz_oxide::inflate::decompress_to_vec_zlib(&e.z).unwrap_or_default();
            data.resize(CARD_LEN, 0);
            let t = &e.trailer;
            Card {
                list: l,
                slot,
                name: utf16(&t[2..24]),
                hr: u16_at(t, 0),
                id: t[24..32].try_into().unwrap(),
                playtime: u32_at(t, 32),
                data,
                info,
            }
        })
        .collect()
}

/// The names of a list's cards, from the trailers (no inflating).
pub fn names(s: &Save, base: usize, l: List) -> Vec<String> {
    elems(s, base, l).unwrap_or_default().into_iter().filter(|(e, _)| e.state == FILLED).map(|(e, _)| utf16(&e.trailer[2..24])).collect()
}

/// Delete the card in `slot`, moving the later cards up as the game does. False when the
/// slot holds no card.
pub fn remove(s: &mut Save, base: usize, l: List, slot: usize) -> bool {
    let Some(v) = elems(s, base, l) else { return false };
    if v.get(slot).is_none_or(|(e, _)| e.state != FILLED) {
        return false;
    }
    let keep = v.into_iter().enumerate().filter(|(i, (e, _))| *i != slot && e.state == FILLED).map(|(_, x)| x).collect();
    write(s, base, l, keep);
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moved {
    /// Added after the stored cards, as a StreetPass card.
    Added,
    /// The stored card of the same hunter had less play time: replaced, date set.
    Updated,
    /// The stored card of the same hunter is as recent: only the inbox card goes.
    Kept,
    /// 100 cards stored and none of this hunter: nothing changed.
    Full,
}

/// Move inbox card `slot` to the stored cards, as the Post Office's *Move* does
/// (`0x16491c`): a stored card of the same name and ID is replaced only by one with more
/// play time (its date set to `today`, Unity, comment and type kept); otherwise the card
/// goes after the stored ones with the inbox's date, type StreetPass and the sender's
/// ID. The inbox card is then deleted.
pub fn move_to_stored(s: &mut Save, base: usize, slot: usize, today: (u8, u8, u16)) -> Option<Moved> {
    let mut inbox = elems(s, base, List::Inbox)?;
    let mut stored = elems(s, base, List::Stored)?;
    let (e, inf) = inbox.get(slot).filter(|(e, _)| e.state == FILLED)?.clone();
    let same = |x: &Elem| x.state == FILLED && x.trailer[2..32] == e.trailer[2..32];
    stored.retain(|(x, _)| x.state == FILLED);
    let moved = match stored.iter().position(|(x, _)| same(x)) {
        Some(i) if u32_at(&stored[i].0.trailer, 32) < u32_at(&e.trailer, 32) => {
            let mut info = stored[i].1.clone();
            info[0] = today.0;
            info[1] = today.1;
            info[2..4].copy_from_slice(&today.2.to_le_bytes());
            stored[i] = (e.clone(), info);
            Moved::Updated
        }
        Some(_) => Moved::Kept,
        None if stored.len() >= List::Stored.at().1 => return Some(Moved::Full),
        None => {
            let mut info = vec![0u8; 44];
            info[..4].copy_from_slice(&inf[..4]);
            info[0x20] = STREETPASS as u8;
            info[0x21..0x29].copy_from_slice(&inf[4..12]);
            stored.push((e.clone(), info));
            Moved::Added
        }
    };
    write(s, base, List::Stored, stored);
    inbox.remove(slot);
    inbox.retain(|(x, _)| x.state == FILLED);
    write(s, base, List::Inbox, inbox);
    Some(moved)
}

// --- blocked-user list (block A) ----------------------------------------------------------

/// The blocked-user list: 100 × 96 B, shared by the characters. A record is a 64-byte ID
/// buffer (40 bytes used: a big-endian `MtNetUniqueId`) and the name, UTF-8, 32 B; an
/// empty first name byte is an empty record. Nothing counts them; the game adds in the
/// first empty record and its delete (`0x1e6248`, `0x1e6698`) moves the later ones up.
pub const BLACKLIST: usize = 0x3F96;
pub const BLOCKED: usize = 100;
const BLOCKED_LEN: usize = 96;
const BLOCKED_NAME: usize = 0x40;

#[derive(Clone, Debug)]
pub struct Blocked {
    /// Its record.
    pub slot: usize,
    pub name: String,
    /// The 40 bytes of the network ID.
    pub id: Vec<u8>,
}

pub fn blacklist(s: &Save) -> Vec<Blocked> {
    (0..BLOCKED)
        .map(|i| BLACKLIST + BLOCKED_LEN * i)
        .enumerate()
        .filter(|&(_, a)| s.u8(a + BLOCKED_NAME) != 0)
        .map(|(slot, a)| {
            let b = s.get(a + BLOCKED_NAME, 32);
            let end = b.iter().position(|&c| c == 0).unwrap_or(32);
            Blocked { slot, name: String::from_utf8_lossy(&b[..end]).into_owned(), id: s.get(a, 40).to_vec() }
        })
        .collect()
}

/// Take record `slot` off the list, moving the later ones up.
pub fn unblock(s: &mut Save, slot: usize) -> bool {
    let all: Vec<Vec<u8>> = (0..BLOCKED).map(|i| s.get(BLACKLIST + BLOCKED_LEN * i, BLOCKED_LEN).to_vec()).collect();
    if all.get(slot).is_none_or(|r| r[BLOCKED_NAME] == 0) {
        return false;
    }
    let mut v: Vec<u8> = all.into_iter().enumerate().filter(|(i, r)| *i != slot && r[BLOCKED_NAME] != 0).flat_map(|(_, r)| r).collect();
    v.resize(BLOCKED * BLOCKED_LEN, 0);
    s.put(BLACKLIST, &v);
    true
}

// --- Hunters for Hire ---------------------------------------------------------------------

/// Hunters for Hire (`GuestHunterMsg`): a 98-byte header, then 13 records of 470 B. The
/// game rerolls the offers at each quest result and builds the hired party's hunters
/// from records 6-9, so the editor only shows them.
pub const GUESTS: usize = 0x11B631;
const GUEST_LEN: usize = 470;
const GUEST_RECORDS: usize = 98;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Offered at the counter now (records 0-2).
    Offered,
    /// The next offer (records 3-5).
    Next,
    /// Hired, coming along on quests (records 6-9).
    Hired,
}

#[derive(Clone, Debug)]
pub struct Guest {
    pub role: Role,
    pub name: String,
    pub title: [u16; 3],
    pub hr: u16,
    /// Weapon: equipment type (7-21) and ID.
    pub weapon: (u8, u16),
    /// Unity with the card's owner; 0 for a hired hunter of the Guild.
    pub unity: u32,
    /// A copy of a stored card (an owner ID), not a "Hired *weapon*" hunter of the Guild.
    pub card: bool,
}

/// Quests left with the hired party, None while no one is hired.
pub fn hired_quests(s: &Save, base: usize) -> Option<u8> {
    (s.u8(base + GUESTS) != 0).then(|| s.u8(base + GUESTS + 1))
}

/// The records the counter counts: offered, next offer, hired (while hiring).
pub fn guests(s: &Save, base: usize) -> Vec<Guest> {
    let h = base + GUESTS;
    let hiring = s.u8(h) != 0;
    let groups = [(Role::Offered, 0, s.u8(h + 2)), (Role::Next, 3, s.u8(h + 3)), (Role::Hired, 6, if hiring { s.u8(h + 5) } else { 0 })];
    let mut out = vec![];
    for (role, first, n) in groups {
        for i in first..first + (n as usize).min(if role == Role::Hired { 4 } else { 3 }) {
            let r = s.get(h + GUEST_RECORDS + GUEST_LEN * i, GUEST_LEN);
            // actor kind 2: no one
            if r[0x70] == 2 {
                continue;
            }
            out.push(Guest {
                role,
                name: utf16(&r[..22]),
                title: [0, 1, 2].map(|k| u16_at(r, 0x16 + 2 * k)),
                hr: u16_at(r, 0x1C),
                weapon: (r[0xA2], u16_at(r, 0xA4)),
                unity: u32_at(r, 0x5C),
                card: r[0x65..0x6D].iter().any(|&b| b != 0),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE};

    fn card(name: &str, id: u8, play: u32, extra: usize) -> Elem {
        let mut d = vec![0u8; CARD_LEN];
        d[0x16] = 7;
        // a card that compresses less, to move the elements around
        for (k, b) in d[0x918..0x918 + extra].iter_mut().enumerate() {
            *b = (k * 7919 % 251) as u8;
        }
        let mut t = vec![0u8; TRAILER];
        t[0] = 7;
        for (k, u) in name.encode_utf16().enumerate() {
            t[2 + 2 * k..4 + 2 * k].copy_from_slice(&u.to_le_bytes());
        }
        t[24] = id;
        t[32..36].copy_from_slice(&play.to_le_bytes());
        Elem { z: miniz_oxide::deflate::compress_to_vec_zlib(&d, 6), state: FILLED, trailer: t }
    }

    #[test]
    fn lists_stay_packed_and_sized() {
        let mut s = blank();
        let b = SLOT1_BASE;
        write(&mut s, b, List::Stored, vec![]);
        write(&mut s, b, List::Inbox, vec![]);
        assert!(cards(&s, b, List::Stored).is_empty());
        let mk = |n, id, p, x, u: u8| (card(n, id, p, x), vec![u; 44]);
        write(&mut s, b, List::Stored, vec![mk("Ann", 1, 10, 300, 1), mk("Bob", 2, 10, 0, 2), mk("Cy", 3, 10, 900, 3)]);
        let before = s.get(b + List::Stored.at().0 + List::Stored.size(), 16).to_vec();
        assert!(remove(&mut s, b, List::Stored, 1));
        let c = cards(&s, b, List::Stored);
        assert_eq!(c.iter().map(|c| (c.slot, c.name.as_str(), c.info[0x10])).collect::<Vec<_>>(), [(0, "Ann", 1), (1, "Cy", 3)]);
        assert_eq!(c[1].data[0x16], 7);
        assert_eq!(s.get(b + List::Stored.at().2 + 2 * 44, 44), &[0; 44], "info records follow");
        assert_eq!(s.get(b + List::Stored.at().0 + List::Stored.size(), 16), before, "nothing past the list");
        assert!(!remove(&mut s, b, List::Stored, 2));

        // inbox: a new hunter, a newer card of a stored one, an older one
        let mut inf = vec![0u8; 36];
        inf[..4].copy_from_slice(&[5, 6, 0xEA, 0x07]);
        inf[4] = 9;
        write(&mut s, b, List::Inbox, vec![(card("Dee", 4, 5, 0), inf.clone()), (card("Cy", 3, 99, 0), inf.clone()), (card("Ann", 1, 1, 0), inf)]);
        assert_eq!(move_to_stored(&mut s, b, 0, (1, 2, 2026)), Some(Moved::Added));
        assert_eq!(move_to_stored(&mut s, b, 0, (1, 2, 2026)), Some(Moved::Updated));
        assert_eq!(move_to_stored(&mut s, b, 0, (1, 2, 2026)), Some(Moved::Kept));
        assert!(cards(&s, b, List::Inbox).is_empty());
        let c = cards(&s, b, List::Stored);
        assert_eq!(c.iter().map(|c| (c.name.as_str(), c.playtime)).collect::<Vec<_>>(), [("Ann", 10), ("Cy", 99), ("Dee", 5)]);
        assert_eq!((c[1].date(), c[1].info[0x10]), ((1, 2, 2026), 3), "updated: date set, the rest kept");
        assert_eq!((c[2].date(), c[2].kind(), c[2].info[0x21]), ((5, 6, 2026), Some(STREETPASS), 9));
    }

    #[test]
    fn blacklist_moves_up() {
        let mut s = blank();
        for (i, n) in ["one", "two", "three"].iter().enumerate() {
            s.put(BLACKLIST + BLOCKED_LEN * i, &[i as u8 + 1; 40]);
            s.set_str(BLACKLIST + BLOCKED_LEN * i + BLOCKED_NAME, 32, n);
        }
        assert!(unblock(&mut s, 0));
        let v = blacklist(&s);
        assert_eq!(v.iter().map(|b| (b.slot, b.name.as_str(), b.id[0])).collect::<Vec<_>>(), [(0, "two", 2), (1, "three", 3)]);
        assert_eq!(s.get(BLACKLIST + 2 * BLOCKED_LEN, BLOCKED_LEN), &[0; BLOCKED_LEN]);
        assert!(!unblock(&mut s, 5));
    }
}
