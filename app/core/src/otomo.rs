//! The character's Palico data beyond the records (sOtomo, docs/11-save-map.md and
//! data/save-fields.csv): the Palico played as Prowler and the hunting buddies, the two
//! counters, the Palico Dojo, the two teams, the StreetPass Palicoes and the scouting
//! request. Every edit is DERIVED (from the game's code and the analysed saves).
//!
//! A Prowler has no Hunter Arts of its own: it fights with the support moves and skills
//! its Palico record has equipped (`palico`).

use crate::data::tables;
use crate::palico::{self, LIST_N};
use crate::save::Save;

/// Palico indices: played as Prowler, hunting buddy 1, hunting buddy 2 (`0x25f880`,
/// `0x25f840`); 0xFF = none.
pub const ROLES: usize = 0x23B9D;
pub const ROLE_N: usize = 3;
pub const NONE: u8 = 0xFF;
/// Palico Dojo sessions completed, at most 100. The awards check after a quest grants
/// award 53 from 50 on (`0x3ed174`, a `>=` test), so the count alone is enough.
pub const DOJO_DONE: usize = 0x23BA0;
pub const DOJO_MAX: u8 = 100;
/// Palicoes hired in total, at most 200 (`0x25e88c`). The title-word check (`0x3f5238`)
/// adds the words from 10, 30, 50 and 80 on, also a `>=` test.
pub const HIRED: usize = 0x23BA1;
pub const HIRED_MAX: u8 = 200;

/// The scouting request (uUIOtomoService), a 20-byte LSB-first bit stream that new-Palico
/// creation follows (`0x25aee0`): fields (id, bit, width). Mode 0 none, 1 by ability (forte,
/// target), 2 by looks; forte k = forte k - 1; target k = the Palico target value k; a look
/// k = its choice k - 1; 0 = any. Four RGBA colours follow at bit 29 (which is which is
/// not proven): left as they are.
pub const REQUEST: usize = 0x23BA2;
pub const REQUEST_FIELDS: [(&str, usize, usize); 8] = [("mode", 0, 2), ("forte", 2, 4), ("target", 6, 3), ("coat", 9, 4), ("eyes", 13, 4), ("ears", 17, 4), ("tail", 21, 4), ("voice", 25, 4)];

/// Palico Dojo teaching session: teacher, students 1-3, kind (1 skill, else support
/// move), skill / move ID, u16 in progress.
pub const TEACHING: usize = 0x2C48E;
/// Palico Dojo training slots, 3 x 16 B: Palico, course parameter, type (4 = none), -,
/// sessions left, sessions booked, type 3: move (0) or skill (1), level when booked,
/// type 3: list slot and ID.
pub const TRAINING: usize = 0x2C49E;
pub const TRAINING_N: usize = 3;
const TRAINING_LEN: usize = 16;
/// Palico Board team (uUIOtomoBoard): u8 count, 5 Palico indices.
pub const BOARD: usize = 0x2C4CE;
pub const BOARD_N: usize = 5;
/// Meownster Hunters (sMonNyan): expedition state, then 4 members of 13 B, s32 Palico
/// index first (-1 none).
pub const EXPEDITION: usize = 0x11CF3D;
pub const MEMBERS: usize = 0x11CF50;
pub const MEMBER_N: usize = 4;
const MEMBER_LEN: usize = 13;

/// StreetPass Palicoes, 276 B: the one to send, then the inbox of 50 with a 36-B info
/// record each.
pub const TO_SEND: usize = 0x11782D;
pub const INBOX: usize = 0x117941;
pub const INBOX_N: usize = 50;
pub const SP_LEN: usize = 276;
pub const INBOX_INFO: usize = 0x11AF29;
pub const INFO_LEN: usize = 36;

fn index(v: u8) -> Option<usize> {
    ((v as usize) < LIST_N).then_some(v as usize)
}

pub fn role(s: &Save, base: usize, r: usize) -> Option<usize> {
    index(s.u8(base + ROLES + r))
}

/// Put Palico `p` (or none) in role `r`; another role that held it is left empty, as one
/// Palico can't be in two. An empty Palico place is refused. Returns the roles changed.
pub fn set_role(s: &mut Save, base: usize, r: usize, p: Option<usize>) -> Vec<usize> {
    if p.is_some_and(|i| i >= LIST_N || palico::is_empty(s, base, i)) {
        return vec![];
    }
    let mut changed = vec![];
    for k in 0..ROLE_N {
        let v = if k == r { p } else if p.is_some() && role(s, base, k) == p { None } else { continue };
        let b = v.map_or(NONE, |i| i as u8);
        if s.u8(base + ROLES + k) != b {
            s.set_u8(base + ROLES + k, b);
            changed.push(k);
        }
    }
    changed
}

pub fn dojo_done(s: &Save, base: usize) -> u8 {
    s.u8(base + DOJO_DONE)
}

pub fn set_dojo_done(s: &mut Save, base: usize, v: u8) {
    s.set_u8(base + DOJO_DONE, v.min(DOJO_MAX));
}

pub fn hired(s: &Save, base: usize) -> u8 {
    s.u8(base + HIRED)
}

pub fn set_hired(s: &mut Save, base: usize, v: u8) {
    s.set_u8(base + HIRED, v.min(HIRED_MAX));
}

/// The highest value of request field `k`.
pub fn request_max(k: usize) -> u8 {
    match REQUEST_FIELDS[k].0 {
        "mode" => 2,
        "forte" => palico::BIASES.len() as u8,
        "target" => palico::TARGETS.len() as u8 - 1,
        look => tables().palico_looks.iter().find(|l| l.look == look).map_or(0, |l| l.choices),
    }
}

pub fn request(s: &Save, base: usize, k: usize) -> u8 {
    let (_, pos, width) = REQUEST_FIELDS[k];
    s.bits(base + REQUEST, pos, width) as u8
}

pub fn set_request(s: &mut Save, base: usize, k: usize, v: u8) {
    let (_, pos, width) = REQUEST_FIELDS[k];
    s.set_bits(base + REQUEST, pos, width, v.min(request_max(k)) as u32);
}

#[derive(Debug, Clone, PartialEq)]
pub struct Training {
    pub palico: usize,
    /// 0-3 (`0x25c554`): 3 teaches a move or skill.
    pub kind: u8,
    pub left: u8,
    pub booked: u8,
    /// Type 3: skill (else support move), its ID.
    pub learns: Option<(bool, u8)>,
}

/// The training slots in use.
pub fn training(s: &Save, base: usize) -> Vec<Training> {
    (0..TRAINING_N)
        .filter_map(|k| {
            let t = s.get(base + TRAINING + TRAINING_LEN * k, TRAINING_LEN);
            Some(Training { palico: index(t[0])?, kind: t[2], left: t[4], booked: t[5], learns: (t[2] == 3).then_some((t[6] == 1, t[9])) })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Teaching {
    pub teacher: usize,
    pub students: Vec<usize>,
    pub skill: bool,
    pub id: u8,
}

/// The teaching session booked, if any.
pub fn teaching(s: &Save, base: usize) -> Option<Teaching> {
    let t = s.get(base + TEACHING, 8);
    Some(Teaching { teacher: index(t[0])?, students: t[1..4].iter().filter_map(|&v| index(v)).collect(), skill: t[4] == 1, id: t[5] })
}

pub fn board(s: &Save, base: usize) -> Vec<usize> {
    let n = (s.u8(base + BOARD) as usize).min(BOARD_N);
    s.get(base + BOARD + 1, n).iter().filter_map(|&v| index(v)).collect()
}

/// 0 at home, 1 out, 2 back with results to collect.
pub fn expedition(s: &Save, base: usize) -> u8 {
    s.u8(base + EXPEDITION)
}

pub fn members(s: &Save, base: usize) -> Vec<usize> {
    (0..MEMBER_N).filter_map(|k| usize::try_from(s.u32(base + MEMBERS + MEMBER_LEN * k) as i32).ok().filter(|&i| i < LIST_N)).collect()
}

/// A StreetPass Palico: the name (UTF-16), then the record's parameter block bytes 0-195
/// from `+0x50` (level - 1 at +4, forte at +5, greeting at +0x40, original owner at +0x7C,
/// sender's hunter name at +0x9C, as in the Palico record from `+0x20`).
#[derive(Debug, Clone, PartialEq)]
pub struct SpPalico {
    pub name: String,
    pub level: u8,
    pub bias: u8,
    pub greeting: String,
    pub owner: String,
    pub sender: String,
}

fn utf16(b: &[u8]) -> String {
    let u: Vec<u16> = b.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&u)
}

fn sp(s: &Save, at: usize) -> Option<SpPalico> {
    let r = s.get(at, SP_LEN);
    (r[8] != 0 || r[9] != 0).then(|| SpPalico {
        name: utf16(&r[8..30]),
        level: r[0x54].saturating_add(1),
        bias: r[0x55],
        greeting: s.str(at + 0x90, 60),
        owner: s.str(at + 0xCC, 32),
        sender: s.str(at + 0xEC, 32),
    })
}

pub fn to_send(s: &Save, base: usize) -> Option<SpPalico> {
    sp(s, base + TO_SEND)
}

/// The inbox's Palicoes with their places.
pub fn inbox(s: &Save, base: usize) -> Vec<(usize, SpPalico)> {
    (0..INBOX_N).filter_map(|k| sp(s, base + INBOX + SP_LEN * k).map(|p| (k, p))).collect()
}

/// Remove inbox Palico `k`: the later ones and their info records move up, the last place
/// is cleared.
pub fn remove_inbox(s: &mut Save, base: usize, k: usize) -> bool {
    if k >= INBOX_N || sp(s, base + INBOX + SP_LEN * k).is_none() {
        return false;
    }
    for (at, len) in [(INBOX, SP_LEN), (INBOX_INFO, INFO_LEN)] {
        let mut v = s.get(base + at, INBOX_N * len).to_vec();
        v.drain(k * len..(k + 1) * len);
        v.resize(INBOX_N * len, 0);
        s.put(base + at, &v);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE as B};

    fn with_palicoes(n: usize) -> Save {
        let mut s = blank();
        for i in 0..n {
            s.set_str(B + palico::offset(i), palico::NAME, &format!("P{i}"));
        }
        s.put(B + ROLES, &[NONE, 2, NONE]);
        s
    }

    #[test]
    fn a_palico_holds_one_role() {
        let mut s = with_palicoes(4);
        assert_eq!((role(&s, B, 0), role(&s, B, 1)), (None, Some(2)));
        // the buddy becomes the Prowler: buddy 1 is left empty
        assert_eq!(set_role(&mut s, B, 0, Some(2)), vec![0, 1]);
        assert_eq!(s.get(B + ROLES, 3), &[2, NONE, NONE]);
        assert!(set_role(&mut s, B, 2, Some(9)).is_empty(), "an empty place");
        assert!(set_role(&mut s, B, 2, Some(LIST_N)).is_empty(), "for hire");
        assert_eq!(set_role(&mut s, B, 0, None), vec![0]);
        assert_eq!(role(&s, B, 0), None);
    }

    #[test]
    fn counters_stop_at_the_game_limits() {
        let mut s = blank();
        set_dojo_done(&mut s, B, 150);
        set_hired(&mut s, B, 250);
        assert_eq!((dojo_done(&s, B), hired(&s, B)), (DOJO_MAX, HIRED_MAX));
    }

    #[test]
    fn request_fields_of_the_analysed_save() {
        let mut s = blank();
        s.put(B + REQUEST, &[0x1d, 0, 0, 0, 0x1e, 0x1e, 0xfe, 0x3f, 0xce, 0xfb, 0xff, 0x3f, 0xce, 0xfb, 0xff, 0x3f, 0xce, 0xfb, 0xff, 0x1f]);
        // by ability, Gathering (6 + 1), any target
        assert_eq!((0..8).map(|k| request(&s, B, k)).collect::<Vec<_>>(), [1, 7, 0, 0, 0, 0, 0, 0]);
        let colours = s.get(B + REQUEST + 3, 17).to_vec();
        set_request(&mut s, B, 0, 2);
        set_request(&mut s, B, 7, 9);
        set_request(&mut s, B, 2, 5);
        assert_eq!((request(&s, B, 0), request(&s, B, 7), request(&s, B, 2), request(&s, B, 1)), (2, 3, 5, 7));
        assert_eq!(&s.get(B + REQUEST + 3, 17)[1..], &colours[1..], "the colours stay");
        assert_eq!((request_max(1), request_max(3)), (8, 7));
    }

    #[test]
    fn dojo_and_teams() {
        let mut s = with_palicoes(4);
        // the analysed save: nothing booked
        s.put(B + TEACHING, &[NONE, NONE, NONE, NONE, 2]);
        for k in 0..TRAINING_N {
            s.put(B + TRAINING + TRAINING_LEN * k, &[NONE, 0, 4]);
        }
        s.put(B + BOARD, &[0, NONE, NONE, NONE, NONE, NONE]);
        assert!(training(&s, B).is_empty() && teaching(&s, B).is_none() && board(&s, B).is_empty());
        s.put(B + TRAINING + TRAINING_LEN, &[3, 0, 3, 0, 2, 5, 1, 0, 11, 21]);
        s.put(B + TEACHING, &[1, 2, NONE, 3, 0, 26]);
        s.put(B + BOARD, &[2, 0, 3, NONE, NONE, NONE]);
        assert_eq!(training(&s, B), vec![Training { palico: 3, kind: 3, left: 2, booked: 5, learns: Some((true, 21)) }]);
        assert_eq!(teaching(&s, B), Some(Teaching { teacher: 1, students: vec![2, 3], skill: false, id: 26 }));
        assert_eq!(board(&s, B), vec![0, 3]);
        for (k, v) in [4u32, 5, u32::MAX, 7].into_iter().enumerate() {
            s.set_u32(B + MEMBERS + MEMBER_LEN * k, v);
        }
        assert_eq!(members(&s, B), vec![4, 5, 7]);
    }

    #[test]
    fn removing_an_inbox_palico_moves_the_rest_up() {
        let mut s = blank();
        for (k, n) in ["Tama", "Kuro", "Mike"].iter().enumerate() {
            let at = B + INBOX + SP_LEN * k;
            for (j, u) in n.encode_utf16().enumerate() {
                s.set_u16(at + 8 + 2 * j, u);
            }
            s.set_u8(at + 0x54, 9);
            s.set_u8(B + INBOX_INFO + INFO_LEN * k, k as u8 + 1);
        }
        assert_eq!(inbox(&s, B).iter().map(|(k, p)| (*k, p.name.as_str(), p.level)).collect::<Vec<_>>(), [(0, "Tama", 10), (1, "Kuro", 10), (2, "Mike", 10)]);
        assert!(remove_inbox(&mut s, B, 1));
        assert_eq!(inbox(&s, B).iter().map(|(_, p)| p.name.as_str()).collect::<Vec<_>>(), ["Tama", "Mike"]);
        assert_eq!(s.get(B + INBOX_INFO, 1 + INFO_LEN)[INFO_LEN], 3);
        assert_eq!(s.u8(B + INBOX_INFO + 2 * INFO_LEN), 0);
        assert!(!remove_inbox(&mut s, B, 2), "an empty place");
        assert!(to_send(&s, B).is_none());
    }
}
