//! Character stats and the copies the game keeps of them (docs/11-save-map.md).
//!
//! The 632-byte slot header (base + 0) is a summary for the slot screen: the loader
//! drops it. The values the game loads live in the player record (name), the S+0x20
//! block (HR points, zenny, Wycademy and village points) and sGameControl (play time).
//! The editor writes the loaded copy and refreshes the summaries so all agree.
//!
//! HR is not stored: the game computes it on every load (0x52139c):
//!   Hub ★ 0 -> HR 0; HR limit not released (progress bit 20) -> min(Hub ★, 12);
//!   else the highest N with HR points >= need(N), at least 13 (999 from 4,246,430).

use crate::save::Save;

pub const HDR_NAME: usize = 0x0;
pub const HDR_PLAYTIME: usize = 0x20;
pub const HDR_FUNDS: usize = 0x24;
pub const HDR_HR: usize = 0x28;
pub const PLAYER_NAME: usize = 0x23B7D;
pub const NAME_LEN: usize = 32;
/// "Names must be 10 characters or less."
pub const NAME_CHARS: usize = 10;
pub const CARD: usize = 0xC71BD;
pub const CARD_NAME_UNITS: usize = 11;
pub const CARD_HR: usize = CARD + 0x16;
pub const CARD_PLAYTIME: usize = CARD + 0x914;
pub const HR_POINTS: usize = 0x280B;
pub const FUNDS: usize = 0x280F;
pub const WYCADEMY: usize = 0x2817;
pub const POINTS_LR: usize = 0x281B;
pub const POINTS_G: usize = 0x282B;
pub const PLAYTIME: usize = 0x2248B;
pub const BODY: usize = 0x23B47;
pub const PROGRESS: usize = 0x2F77;
pub const PROGRESS_NEW: usize = 0x2F7F;
pub const HR_RELEASED_BIT: usize = 20;
pub const HUB_STAR: usize = 0x2C4DC;
pub const MAX_FUNDS: u32 = 9_999_999;
pub const MAX_POINTS: u32 = 9_999_999;
pub const MAX_VILLAGE_POINTS: u32 = 20_000;
pub const MAX_PLAYTIME: u32 = 35_999_999;

/// The character-creation block (12 B, `BODY` in the player record, the copy the game
/// loads; summaries in the slot header and the own Guild Card) and its 9 RGBA colours
/// right after it. DERIVED (docs/11-save-map.md, "Hunter appearance"): +0 weapon class,
/// +1 voice (1-20, player/com/<m|f>/vo/01-20), +2 face (18 models), +3 clothing, +4 gender,
/// +5 hunting style, +6 hairstyle (30 models), +8 features; colours 0-4 the armour
/// pigment, 5 skin, 6 hair, 7 features, 8 eyes.
pub const LOOKS: [usize; 3] = [BODY, 0x240, CARD + 0x18];
pub const LOOK_COLOURS: [usize; 3] = [BODY + 12, 0x24C, CARD + 0x24];
pub const LOOK_VOICE: usize = 1;
pub const LOOK_FACE: usize = 2;
pub const LOOK_CLOTHING: usize = 3;
pub const LOOK_GENDER: usize = 4;
pub const LOOK_HAIR: usize = 6;
pub const LOOK_FEATURES: usize = 8;
pub const COLOUR_SKIN: usize = 5;
pub const COLOUR_HAIR: usize = 6;
pub const COLOUR_FEATURES: usize = 7;
pub const COLOUR_EYES: usize = 8;
pub const FACES: u8 = 18;
pub const HAIRSTYLES: u8 = 30;
pub const VOICES: u8 = 20;

/// Byte `k` of the creation block.
pub fn look(s: &Save, base: usize, k: usize) -> u8 {
    s.u8(base + BODY + k)
}

/// Write byte `k` of the creation block to all three copies.
pub fn set_look(s: &mut Save, base: usize, k: usize, v: u8) {
    for a in LOOKS {
        s.set_u8(base + a + k, v);
    }
}

/// Colour slot `c` as RGB.
pub fn look_colour(s: &Save, base: usize, c: usize) -> [u8; 3] {
    let v = s.get(base + BODY + 12 + 4 * c, 3);
    [v[0], v[1], v[2]]
}

/// Write colour slot `c` (alpha 0xFF) to all three copies.
pub fn set_look_colour(s: &mut Save, base: usize, c: usize, [r, g, b]: [u8; 3]) {
    for a in LOOK_COLOURS {
        s.put(base + a + 4 * c, &[r, g, b, 0xFF]);
    }
}

/// HR points for HR 1-51 (table 0x162c058).
const NEED_LOW: [u32; 51] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32050, 33470, 34960, 36520, 38150, 39850, 41620, 43460, 45370, 47350, 49400, 51520,
    53710, 55970, 58300, 60700, 63170, 65710, 68320, 71000, 73750, 76570, 79460, 82420, 85450, 88550, 91720, 94960, 98270,
    101650, 105100, 108620, 112210, 115870, 119600, 123400, 127270, 131210,
];
/// HR 52-999: (first HR, points at it, points per HR) of the game's linear bands (0x523390).
const BANDS: [(u32, u32, u32); 10] = [
    (52, 135220, 4010),
    (102, 335790, 4080),
    (202, 743860, 4150),
    (302, 1158930, 4220),
    (402, 1581000, 4290),
    (502, 2010070, 4360),
    (602, 2446140, 4430),
    (702, 2889210, 4500),
    (802, 3339280, 4570),
    (902, 3796350, 4640),
];

/// HR points needed for HR `n` (1-999).
pub fn hr_need(n: u16) -> u32 {
    let n = n.clamp(1, 999) as u32;
    if n <= 51 {
        return NEED_LOW[n as usize - 1];
    }
    let &(from, at, per) = BANDS.iter().rev().find(|b| b.0 <= n).unwrap();
    at + (n - from) * per
}

/// The game's HR rule.
pub fn hr_of(hub_star: u16, released: bool, points: u32) -> u16 {
    if hub_star == 0 {
        0
    } else if !released {
        hub_star.min(12)
    } else {
        (13..=999u16).rev().find(|&n| points >= hr_need(n)).unwrap_or(13)
    }
}

pub struct Stats {
    pub name: String,
    /// As the game computes it on load.
    pub hr: u16,
    pub hr_released: bool,
    pub hr_points: u32,
    pub funds: u32,
    pub wycademy: u32,
    pub playtime: u32,
    pub points_lr: [u32; 4],
    pub points_g: [u32; 4],
    pub gender: u8,
    pub weapon_class: u8,
}

pub fn get(s: &Save, base: usize) -> Stats {
    let pts = |o: usize| std::array::from_fn(|v| s.u32(base + o + 4 * v));
    let released = s.bit(base + PROGRESS, HR_RELEASED_BIT);
    let hr_points = s.u32(base + HR_POINTS);
    Stats {
        name: s.str(base + PLAYER_NAME, NAME_LEN),
        hr: hr_of(s.u16(base + HUB_STAR), released, hr_points),
        hr_released: released,
        hr_points,
        funds: s.u32(base + FUNDS),
        wycademy: s.u32(base + WYCADEMY),
        playtime: s.u32(base + PLAYTIME),
        points_lr: pts(POINTS_LR),
        points_g: pts(POINTS_G),
        gender: s.u8(base + BODY + 4),
        weapon_class: s.u8(base + BODY),
    }
}

/// Name in every copy: player record (loaded), slot header and the own Guild Card
/// (UTF-16). At most 10 characters and 31 bytes.
pub fn set_name(s: &mut Save, base: usize, name: &str) {
    let mut name: String = name.chars().filter(|c| (*c as u32) < 0x10000).take(NAME_CHARS).collect();
    while name.len() > NAME_LEN - 1 {
        name.pop();
    }
    s.set_str(base + HDR_NAME, NAME_LEN, &name);
    s.set_str(base + PLAYER_NAME, NAME_LEN, &name);
    let mut w = vec![0u8; 2 * CARD_NAME_UNITS];
    for (k, u) in name.encode_utf16().take(CARD_NAME_UNITS).enumerate() {
        w[2 * k..2 * k + 2].copy_from_slice(&u.to_le_bytes());
    }
    s.put(base + CARD, &w);
}

fn refresh_hr_copies(s: &mut Save, base: usize) {
    let hr = get(s, base).hr;
    s.set_u16(base + HDR_HR, hr);
    s.set_u16(base + CARD_HR, hr);
}

/// Set HR `n` the way the game reaches it. 13-999: release the HR limit (both progress
/// maps, as talk action 5 does) and set the points to the threshold. Below 13 the HR
/// follows the Hub star level, so that is what is returned unchanged: the caller
/// should edit the Hub star instead.
pub fn set_hr(s: &mut Save, base: usize, n: u16) -> bool {
    if n < 13 || s.u16(base + HUB_STAR) == 0 {
        return false;
    }
    s.set_bit(base + PROGRESS, HR_RELEASED_BIT, true);
    s.set_bit(base + PROGRESS_NEW, HR_RELEASED_BIT, true);
    s.set_u32(base + HR_POINTS, hr_need(n));
    refresh_hr_copies(s, base);
    true
}

pub fn set_hr_points(s: &mut Save, base: usize, v: u32) {
    s.set_u32(base + HR_POINTS, v.min(MAX_POINTS));
    refresh_hr_copies(s, base);
}

pub fn set_hub_star(s: &mut Save, base: usize, v: u16) {
    s.set_u16(base + HUB_STAR, v.min(13));
    refresh_hr_copies(s, base);
}

pub fn set_funds(s: &mut Save, base: usize, v: u32) {
    let v = v.min(MAX_FUNDS);
    s.set_u32(base + FUNDS, v);
    s.set_u32(base + HDR_FUNDS, v);
}

pub fn set_wycademy(s: &mut Save, base: usize, v: u32) {
    s.set_u32(base + WYCADEMY, v.min(MAX_POINTS));
}

/// Village points of village `v` (Bherna, Kokoto, Pokke, Yukumo), low or G rank.
pub fn set_village_points(s: &mut Save, base: usize, v: usize, g: bool, pts: u32) {
    assert!(v < 4, "village {v} out of range");
    let o = if g { POINTS_G } else { POINTS_LR };
    s.set_u32(base + o + 4 * v, pts.min(MAX_VILLAGE_POINTS));
}

/// Guild Card weapon usage (docs/04, CONFIRMED): quests completed with each weapon, one
/// array of 15 u16 per venue. The game shows their sums and picks the main weapon from
/// them; nothing else depends on them.
pub const WEAPON_USE: usize = CARD + 0x8BA;
// i18n: shown through tr() in the GUI
pub const VENUES: [&str; 3] = ["Village", "Hub", "Arena"];
/// Storage order (the classic internal one).
// i18n: shown through tr() in the GUI
pub const USE_WEAPONS: [&str; 15] = [
    "Great Sword", "Sword and Shield", "Hammer", "Lance", "Heavy Bowgun", "Light Bowgun", "Long Sword", "Switch Axe",
    "Gunlance", "Bow", "Dual Blades", "Hunting Horn", "Insect Glaive", "Charge Blade", "Prowler",
];
/// The order the Guild Card draws them in, as storage indices.
pub const USE_SHOWN: [usize; 15] = [0, 6, 1, 10, 2, 11, 3, 8, 7, 13, 12, 5, 4, 9, 14];
pub const MAX_USE: u16 = 9999;

fn use_at(base: usize, venue: usize, w: usize) -> usize {
    assert!(venue < 3 && w < 15, "weapon usage {venue}/{w}");
    base + WEAPON_USE + 2 * (15 * venue + w)
}

pub fn weapon_use(s: &Save, base: usize, venue: usize, w: usize) -> u16 {
    s.u16(use_at(base, venue, w))
}

pub fn set_weapon_use(s: &mut Save, base: usize, venue: usize, w: usize, v: u16) {
    s.set_u16(use_at(base, venue, w), v.min(MAX_USE));
}

pub fn set_playtime(s: &mut Save, base: usize, secs: u32) {
    let secs = secs.min(MAX_PLAYTIME);
    s.set_u32(base + PLAYTIME, secs);
    s.set_u32(base + HDR_PLAYTIME, secs);
    s.set_u32(base + CARD_PLAYTIME, secs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE};

    #[test]
    fn looks_reach_every_copy() {
        let mut s = blank();
        set_look(&mut s, SLOT1_BASE, LOOK_HAIR, 26);
        set_look_colour(&mut s, SLOT1_BASE, COLOUR_SKIN, [0xe9, 0xd6, 0xcc]);
        for (a, c) in LOOKS.into_iter().zip(LOOK_COLOURS) {
            assert_eq!(s.u8(SLOT1_BASE + a + LOOK_HAIR), 26);
            assert_eq!(s.get(SLOT1_BASE + c + 4 * COLOUR_SKIN, 4), [0xe9, 0xd6, 0xcc, 0xff]);
        }
        assert_eq!((look(&s, SLOT1_BASE, LOOK_HAIR), look_colour(&s, SLOT1_BASE, COLOUR_SKIN)), (26, [0xe9, 0xd6, 0xcc]));
        // the block is the player record's 12 bytes and the colours follow it
        assert_eq!(get(&s, SLOT1_BASE).gender, 0);
    }

    #[test]
    fn weapon_usage_at_the_documented_offsets() {
        let mut s = blank();
        // docs/04 worked example: Charge Blade, Hub = 0x25474B for slot 1
        set_weapon_use(&mut s, SLOT1_BASE, 1, 13, 12_000);
        assert_eq!(s.u16(0x25474B), MAX_USE);
        assert_eq!(weapon_use(&s, SLOT1_BASE, 1, 13), MAX_USE);
        assert_eq!(use_at(SLOT1_BASE, 2, 0), 0x25474F);
        let mut shown = USE_SHOWN;
        shown.sort();
        assert_eq!(shown, std::array::from_fn(|i| i), "every weapon once");
    }

    #[test]
    fn hr_table() {
        assert_eq!(hr_need(14), 32050);
        assert_eq!(hr_need(100), 327_700);
        assert_eq!(hr_need(500), 2_001_420);
        assert_eq!(hr_need(999), 4_246_430);
        assert_eq!(hr_of(10, false, 0), 10);
        assert_eq!(hr_of(13, false, 9_999_999), 12);
        assert_eq!(hr_of(13, true, 0), 13);
        assert_eq!(hr_of(13, true, 327_700), 100);
        assert_eq!(hr_of(13, true, 327_699), 99);
        assert_eq!(hr_of(13, true, 9_999_999), 999);
        assert_eq!(hr_of(0, true, 9_999_999), 0);
    }
}
