//! Game assets embedded at build time from app/assets/gen (made by tools/build_assets.py
//! from a RomFS dump; not in git). Without them the UI shows placeholders and "#ID".

use mhgu_save::items::{self, Store};
use serde::Deserialize;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

fn file(name: &str) -> Option<&'static [u8]> {
    FILES.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

pub fn available() -> bool {
    file("names.json").is_some()
}

/// One equipment piece of the game's tables.
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Piece {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub rarity: u32,
    /// Weapons: name below max level, at max level (final), past the limit (ultimate).
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub max_lv: u32,
    #[serde(default)]
    pub lim: u32,
    /// Armor: wearable by hunter type 1 (male) / type 2 (female); absent for weapons.
    pub male: Option<u8>,
    pub female: Option<u8>,
    /// Armor: a Blademaster / Gunner piece (both for either). DERIVED.
    pub blade: Option<u8>,
    pub gunner: Option<u8>,
    /// Armor: decoration slots. DERIVED.
    pub slots: Option<u8>,
    /// Weapons: decoration slots at level 1, 2, … DERIVED.
    #[serde(default)]
    pub level_slots: Vec<u8>,
}

impl Piece {
    /// A real piece rather than the tables' ID-0 and DUMMY placeholders.
    pub fn is_real(&self) -> bool {
        self.id != 0 && !self.name.is_empty() && self.name != "DUMMY" && self.name != "(None)"
    }
}

impl Piece {
    /// The weapon's name at in-game level `lv` (0x126dd8).
    pub fn name_at(&self, lv: u32) -> &str {
        let k = if self.names.len() < 3 || lv < self.max_lv { 0 } else if lv < self.max_lv + self.lim { 1 } else { 2 };
        self.names.get(k).map(String::as_str).filter(|s| !s.is_empty()).unwrap_or(&self.name)
    }
}

#[derive(Deserialize, Default)]
pub struct Names {
    #[serde(default)]
    pub items: Vec<String>,
    /// Weapon class 0-14 -> pieces by equipment ID.
    #[serde(default)]
    pub weapons: HashMap<String, Vec<Piece>>,
    /// Armor part 1-5 -> pieces by armor ID.
    #[serde(default)]
    pub armor: HashMap<String, Vec<Piece>>,
    #[serde(default)]
    pub palico_weapons: Vec<Piece>,
    #[serde(default)]
    pub palico_armor: HashMap<String, Vec<Piece>>,
    /// Talisman skill tree ID -> name.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Palico support move ID -> name (packs built before 2026-10-05 lack it: shown as IDs).
    #[serde(default)]
    pub support_moves: Vec<String>,
    /// Item ID -> [icon, colour, rarity].
    #[serde(default)]
    pub item_icons: HashMap<String, [u32; 3]>,
    /// Item ID -> pouch carry limit (packs built before 2026-10-05 lack it: 99 for all).
    #[serde(default)]
    pub item_carry: Vec<u8>,
    /// Item colour index -> RGB.
    #[serde(default)]
    pub palette: Vec<[u8; 3]>,
    /// Rarity -> RGB (equipment icon tint).
    #[serde(default)]
    pub rarity_colors: Vec<[u8; 3]>,
    /// Icon id -> top-left of its cell on `items.png`.
    #[serde(default)]
    pub icon_rects: Vec<[u32; 2]>,
    #[serde(default)]
    pub icon_cell: u32,
    /// Equipment type code (1-24) -> icon id.
    #[serde(default)]
    pub equip_icons: HashMap<String, u32>,
    /// Talisman equipment ID -> name.
    #[serde(default)]
    pub talismans: Vec<Piece>,
    /// Decorations: [item ID, slots it takes] (packs built before 2026-10-06 lack it).
    #[serde(default)]
    pub decos: Vec<[u16; 2]>,
}

/// Slots decoration item `id` takes; None when it is not a decoration (or the pack is
/// older).
pub fn deco_size(id: u16) -> Option<u8> {
    names().decos.iter().find(|d| d[0] == id).map(|d| d[1] as u8)
}

pub fn names() -> &'static Names {
    static N: OnceLock<Names> = OnceLock::new();
    N.get_or_init(|| file("names.json").and_then(|b| serde_json::from_slice(b).ok()).unwrap_or_default())
}

pub fn item_name(id: u16) -> String {
    match names().items.get(id as usize) {
        Some(n) if !n.is_empty() => n.clone(),
        _ => format!("Item #{id}"),
    }
}

/// Most of item `id` one slot of `store` holds: 99 in the box, the item's carry limit
/// in the pouch (itemData +6).
pub fn item_max(id: u16, store: Store) -> u8 {
    match store {
        Store::Box => items::MAX_COUNT,
        Store::Pouch => names().item_carry.get(id as usize).map_or(items::MAX_COUNT, |&c| c.clamp(1, items::MAX_COUNT)),
    }
}

fn decode(png: &[u8]) -> Option<image::RgbaImage> {
    image::load_from_memory_with_format(png, image::ImageFormat::Png).ok().map(|i| i.to_rgba8())
}

fn to_slint(img: &image::RgbaImage) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(img.as_raw(), img.width(), img.height()))
}

thread_local! {
    static MONSTERS: RefCell<HashMap<usize, Option<Image>>> = RefCell::new(HashMap::new());
    static ITEMS: RefCell<HashMap<(u32, u32), Option<Image>>> = RefCell::new(HashMap::new());
    static SHEET: RefCell<Option<Option<image::RgbaImage>>> = const { RefCell::new(None) };
}

pub fn monster_icon(i: usize) -> Option<Image> {
    MONSTERS.with_borrow_mut(|m| m.entry(i).or_insert_with(|| file(&format!("monsters/{i}.png")).and_then(decode).map(|i| to_slint(&i))).clone())
}

/// Icon `icon` of the grayscale sheet multiplied by `rgb` (the game tints its icons).
pub fn sheet_icon(icon: u32, rgb: [u8; 3]) -> Option<Image> {
    let key = (icon, u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 0]));
    ITEMS.with_borrow_mut(|m| {
        m.entry(key)
            .or_insert_with(|| {
                let n = names();
                let [x, y] = *n.icon_rects.get(icon as usize)?;
                let cell = n.icon_cell;
                SHEET.with_borrow_mut(|s| {
                    let sheet = s.get_or_insert_with(|| file("items.png").and_then(decode)).as_ref()?;
                    if x + cell > sheet.width() || y + cell > sheet.height() {
                        return None;
                    }
                    let mut out = image::imageops::crop_imm(sheet, x, y, cell, cell).to_image();
                    for p in out.pixels_mut() {
                        for c in 0..3 {
                            p[c] = (p[c] as u32 * rgb[c] as u32 / 255) as u8;
                        }
                    }
                    Some(to_slint(&out))
                })
            })
            .clone()
    })
}

pub fn item_icon(id: u16) -> Option<Image> {
    let n = names();
    let [icon, colour, _] = *n.item_icons.get(&id.to_string())?;
    let rgb = n.palette.get(colour as usize).copied().unwrap_or([255, 255, 255]);
    sheet_icon(icon, rgb)
}

pub fn rarity_rgb(r: u32) -> [u8; 3] {
    names().rarity_colors.get(r as usize).copied().unwrap_or([200, 200, 200])
}

pub fn award_icon(bit: usize) -> Option<Image> {
    file(&format!("awards/{bit}.png")).and_then(decode).map(|i| to_slint(&i))
}

pub fn equip_icon(type_code: u8, rarity: u32) -> Option<Image> {
    let icon = *names().equip_icons.get(&type_code.to_string())?;
    sheet_icon(icon, rarity_rgb(rarity))
}
