//! Options and chat: the per-character game options (`sGameControl +0x5e`, the Start
//! Menu's *Options* and *Multiplayer Settings* windows), the control options of the
//! character data, the chat phrases, and the title menu's settings shared by the three
//! characters (docs/11-save-map.md, "Smaller managers", "Block A header"). DERIVED from
//! the option windows' item, get and set functions (`0x5e7558`, `0x5e7d34`, `0x5e8130`,
//! network `0x5e947c`) run under emulation, and the chat loader `0x1caab8`.

use crate::save::Save;

/// How an option's choices read in game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choices {
    /// 0 On, 1 Off.
    OnOff,
    /// 0 Off, 1-6, 7 Max.
    Volume,
    /// Type 1 … Type n.
    Types(u8),
    /// Normal, Flip Y, Flip X, Flip X+Y.
    Flip,
    /// Slow, Default, Fast.
    Speed,
    /// Auto, Manual.
    AutoManual,
    /// Manual, Semi-auto.
    ManualSemi,
}

impl Choices {
    pub fn count(self) -> u8 {
        match self {
            Choices::OnOff | Choices::AutoManual | Choices::ManualSemi => 2,
            Choices::Volume => 8,
            Choices::Types(n) => n,
            Choices::Flip => 4,
            Choices::Speed => 3,
        }
    }
}

/// An option byte: its id, offset from the character base, choices and the game's
/// default (table `0x162cda8`; the control bytes reset to 0). The stored value is the
/// choice's index.
#[derive(Clone, Copy, Debug)]
pub struct Opt {
    pub id: &'static str,
    pub at: usize,
    pub choices: Choices,
    pub default: u8,
}

const fn o(id: &'static str, at: usize, choices: Choices, default: u8) -> Opt {
    Opt { id, at, choices, default }
}

/// In the windows' order: *Options* pages 1-3, then *Multiplayer Settings*. Left out: the
/// hidden Sound Settings (`+0x2`) and START: Kick (`+0x10`, still read by the menu bar),
/// the Circle Pad Pro working copy (`+0x14`, rebuilt at load), an unused 3DS byte
/// (`+0x15`), the L Stick option (`+0x43fb`, its text is in the update only) and the
/// Shoutout reset selection (`+0x1c`, only what a reset clears).
pub const OPTIONS: [Opt; 28] = [
    o("music", 0x2246E, Choices::Volume, 5),
    o("sfx", 0x2246F, Choices::Volume, 7),
    o("hud", 0x22471, Choices::OnOff, 0),
    o("map", 0x22472, Choices::OnOff, 0),
    o("camera-angle", 0x22473, Choices::Types(5), 3),
    o("camera-controls", 0x22474, Choices::Flip, 0),
    o("camera-speed", 0x22475, Choices::Speed, 0),
    o("scope", 0x22476, Choices::Flip, 0),
    o("bowgun", 0x22477, Choices::Types(3), 1),
    o("quick-aim", 0x22478, Choices::Flip, 0),
    o("quick-aim-camera", 0x22479, Choices::Types(5), 0),
    o("reticle", 0x2247A, Choices::Speed, 1),
    o("orientation", 0x2247B, Choices::Types(2), 0),
    o("bow", 0x2247C, Choices::Types(2), 1),
    o("bow-cancel", 0x2247D, Choices::AutoManual, 0),
    o("melody", 0x22489, Choices::Types(2), 0),
    o("target-controls", 0x2247F, Choices::Types(2), 0),
    o("target-behavior", 0x22480, Choices::Types(2), 0),
    o("terrain-camera", 0x22481, Choices::ManualSemi, 1),
    o("lens-flares", 0x22484, Choices::OnOff, 0),
    // the character data's control options (S +0x446c)
    o("control-type", 0x5EA2, Choices::Types(4), 0),
    o("plus-button", 0x5EA3, Choices::Types(2), 0),
    o("lr-buttons", 0x5EA4, Choices::Types(2), 0),
    o("r-stick", 0x5EA5, Choices::Types(2), 0),
    // Multiplayer Settings
    o("net-cards", 0x22485, Choices::OnOff, 0),
    o("net-chat", 0x22486, Choices::OnOff, 0),
    o("net-invites", 0x22487, Choices::OnOff, 0),
    o("net-palico-chat", 0x22488, Choices::OnOff, 0),
];

pub fn option(s: &Save, base: usize, k: usize) -> u8 {
    s.u8(base + OPTIONS[k].at)
}

/// Set option `k` to a choice it has; false (nothing written) for one it lacks.
pub fn set_option(s: &mut Save, base: usize, k: usize, v: u8) -> bool {
    let o = OPTIONS[k];
    if v >= o.choices.count() {
        return false;
    }
    s.set_u8(base + o.at, v);
    true
}

// --- chat -------------------------------------------------------------------------------

/// Three chat groups (*Group 1-3*, *Switch Group*), each 24 shortcut phrases and 9
/// auto-shoutout lines, in 104-byte UTF-8 slots: group g's phrase i is slot 24g + i (auto
/// 9g + i). New characters and *Reset Shoutout Groups* copy the game's default phrases
/// into them in the game's language; an empty slot is sent empty.
pub const SHOUTOUTS: usize = 0x11D089;
pub const AUTO: usize = 0x11EDC9;
pub const GROUPS: usize = 3;
pub const PER_GROUP: usize = 24;
pub const AUTO_PER_GROUP: usize = 9;
const PHRASE: usize = 104;
/// The in-game keyboard's limit (`0x1c9dd0`).
pub const PHRASE_CHARS: usize = 26;
/// The group in use, u16 0-2.
pub const CHAT_GROUP: usize = 0x11F8C1;
/// u32: bit 9g + i = auto line i of group g on. With bit 31 clear the loader replaces it
/// by the default `0xF8FC7E3F` (lines 1-6 on in each group, bits 27-31 the marker).
pub const AUTO_ON: usize = 0x5057;
const AUTO_DEFAULT: u32 = 0xF8FC_7E3F;

fn phrase_at(auto: bool, g: usize, i: usize) -> usize {
    if auto {
        assert!(g < GROUPS && i < AUTO_PER_GROUP);
        AUTO + PHRASE * (AUTO_PER_GROUP * g + i)
    } else {
        assert!(g < GROUPS && i < PER_GROUP);
        SHOUTOUTS + PHRASE * (PER_GROUP * g + i)
    }
}

/// Offset (from the base) and length of a phrase slot.
pub fn phrase_slot(auto: bool, g: usize, i: usize) -> (usize, usize) {
    (phrase_at(auto, g, i), PHRASE)
}

pub fn phrase(s: &Save, base: usize, auto: bool, g: usize, i: usize) -> String {
    s.str(base + phrase_at(auto, g, i), PHRASE)
}

/// Write a phrase cut to 26 characters, NUL-padded.
pub fn set_phrase(s: &mut Save, base: usize, auto: bool, g: usize, i: usize, text: &str) {
    let t: String = text.chars().filter(|c| !c.is_control()).take(PHRASE_CHARS).collect();
    s.set_str(base + phrase_at(auto, g, i), PHRASE, &t);
}

pub fn chat_group(s: &Save, base: usize) -> u16 {
    s.u16(base + CHAT_GROUP)
}

pub fn set_chat_group(s: &mut Save, base: usize, g: u16) {
    s.set_u16(base + CHAT_GROUP, g.min(GROUPS as u16 - 1));
}

fn auto_flags(s: &Save, base: usize) -> u32 {
    let v = s.u32(base + AUTO_ON);
    // what the loader turns it into
    if v & 1 << 31 == 0 { AUTO_DEFAULT } else { v }
}

pub fn auto_on(s: &Save, base: usize, g: usize, i: usize) -> bool {
    auto_flags(s, base) & 1 << (AUTO_PER_GROUP * g + i) != 0
}

pub fn set_auto_on(s: &mut Save, base: usize, g: usize, i: usize, on: bool) {
    assert!(g < GROUPS && i < AUTO_PER_GROUP);
    let b = 1u32 << (AUTO_PER_GROUP * g + i);
    let v = auto_flags(s, base);
    s.set_u32(base + AUTO_ON, if on { v | b } else { v & !b });
}

// --- title menu settings (block A, shared) -------------------------------------------------

/// TV brightness: the game scales by 0.4 + 0.025 × value; its slider gives 0-48, 24 = 1.0.
pub const BRIGHTNESS: usize = 0x4C;
pub const MAX_BRIGHTNESS: u8 = 48;
/// Rumble: 1 on, anything else off.
pub const RUMBLE: usize = 0x4D;
/// Text language, the MT language index: 1 English, 2 French, 3 Spanish, 4 German,
/// 5 Italian (the title menu's choices); 0 = not set, the loader takes the console's.
pub const LANGUAGE: usize = 0xB2A4;
pub const LANGUAGES: [u8; 6] = [0, 1, 2, 3, 4, 5];

pub fn set_brightness(s: &mut Save, v: u8) {
    s.set_u8(BRIGHTNESS, v.min(MAX_BRIGHTNESS));
}

pub fn rumble(s: &Save) -> bool {
    s.u8(RUMBLE) == 1
}

pub fn set_rumble(s: &mut Save, on: bool) {
    s.set_u8(RUMBLE, on as u8);
}

/// Set the language to one of `LANGUAGES`; false for any other value.
pub fn set_language(s: &mut Save, v: u8) -> bool {
    if !LANGUAGES.contains(&v) {
        return false;
    }
    s.set_u8(LANGUAGE, v);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{blank, SLOT1_BASE};

    #[test]
    fn options_and_chat_keep_to_the_game() {
        let mut s = blank();
        let b = SLOT1_BASE;
        let cam = OPTIONS.iter().position(|o| o.id == "camera-angle").unwrap();
        assert!(set_option(&mut s, b, cam, 4));
        assert!(!set_option(&mut s, b, cam, 5));
        assert_eq!(option(&s, b, cam), 4);
        // every option byte is its own
        let mut at: Vec<usize> = OPTIONS.iter().map(|o| o.at).collect();
        at.sort();
        at.dedup();
        assert_eq!(at.len(), OPTIONS.len());

        set_phrase(&mut s, b, false, 2, 23, "abcdefghijklmnopqrstuvwxyz0123");
        assert_eq!(phrase(&s, b, false, 2, 23), "abcdefghijklmnopqrstuvwxyz");
        assert_eq!(s.u8(b + CHAT_GROUP - 1), 0, "last slot ends before the group");
        set_phrase(&mut s, b, true, 0, 0, "ÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉÉ");
        assert_eq!(phrase(&s, b, true, 0, 0).chars().count(), 26);

        // a never-initialised value reads as the loader's default and stays marked
        assert!(auto_on(&s, b, 1, 5) && !auto_on(&s, b, 1, 6));
        set_auto_on(&mut s, b, 1, 6, true);
        assert_eq!(s.u32(b + AUTO_ON), AUTO_DEFAULT | 1 << 15);
        set_chat_group(&mut s, b, 7);
        assert_eq!(chat_group(&s, b), 2);

        set_brightness(&mut s, 90);
        assert_eq!(s.u8(BRIGHTNESS), 48);
        assert!(!set_language(&mut s, 7));
        assert!(set_language(&mut s, 4));
    }
}
