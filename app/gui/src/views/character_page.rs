//! The Character page: its models and the callbacks that edit it.

use super::*;
use mhgu_save::arena;
use mhgu_save::guildcard as gc;

pub(super) fn character_page(ui: &AppWindow, st: &State) {
    let s = st.save();
    let c = character::get(s, st.base());
    let p = Char::new(s, st.slot);
    let was = |t: Target| -> SharedString { st.was(t).into() };
    ui.global::<Api>().set_character(CharacterInfo {
        name: c.name.into(),
        hr: c.hr as i32,
        hr_points: c.hr_points as i32,
        funds: c.funds as i32,
        wycademy: c.wycademy as i32,
        playtime_h: (c.playtime / 3600) as i32,
        playtime_m: (c.playtime / 60 % 60) as i32,
        village_star: p.village_star() as i32,
        hub_star: p.hub_star() as i32,
        points_lr: model(c.points_lr.iter().map(|&v| v as i32).collect()),
        points_g: model(c.points_g.iter().map(|&v| v as i32).collect()),
        was_name: was(Target::Name),
        was_hr: was(Target::Hr),
        was_hr_points: was(Target::HrPoints),
        was_funds: was(Target::Funds),
        was_wycademy: was(Target::Wycademy),
        was_playtime: was(Target::Playtime),
        was_village_star: was(Target::VillageStar),
        was_hub_star: was(Target::HubStar),
        was_lr: model((0..4).map(|v| was(Target::Points(v, false))).collect()),
        was_g: model((0..4).map(|v| was(Target::Points(v, true))).collect()),
        weapon_use: model(weapon_use_rows(st)),
        style_use: model((0..character::STYLES.len()).map(|k| character::style_use(s, st.base(), k) as i32).collect()),
        was_style: model((0..character::STYLES.len()).map(|k| was(Target::StyleUse(k))).collect()),
    });
    guild_card(ui, st);
    appearance(ui, st);
}

/// The hunter's creation choices and colours, in the game's terms.
// the colours the game keeps (hair, features and eyes come from elsewhere)
const LOOK_COLOURS: [(&str, usize); 2] = [("skin", character::COLOUR_SKIN), ("clothing", character::COLOUR_CLOTHING)];

fn appearance(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let (s, base) = (st.save(), st.base());
    let types = |n: u8, first: u8| strings((first..first + n).map(|k| trf("Type {}", &[&k])));
    api.set_look_faces(types(character::FACES, 1));
    api.set_look_hairstyles(types(character::HAIRSTYLES, 1));
    api.set_look_voices(types(character::VOICES, 1));
    api.set_look_clothing(types(character::CLOTHING, 1));
    // the game's list ends with None
    api.set_look_features(strings((1..character::FEATURES).map(|k| trf("Type {}", &[&k])).chain([tr("None").to_string()])));
    let l = |k| character::look(s, base, k) as i32;
    let warn = |t: Target| -> SharedString { crate::warnings::of(t, s, st.slot).unwrap_or_default().into() };
    api.set_appearance(AppearanceInfo {
        gender: l(character::LOOK_GENDER),
        face: l(character::LOOK_FACE),
        hair: l(character::LOOK_HAIR),
        features: l(character::LOOK_FEATURES),
        voice: l(character::LOOK_VOICE),
        clothing: l(character::LOOK_CLOTHING),
        colours: model(
            LOOK_COLOURS
                .iter()
                .map(|&(key, c)| {
                    let [r, g, b] = character::look_colour(s, base, c);
                    ColourRow {
                        key: key.into(),
                        label: match key {
                            "skin" => tr("Skin Tone"),
                            _ => tr("Clothing Color"),
                        }
                        .into(),
                        colour: slint::Color::from_rgb_u8(r, g, b),
                        hex: format!("#{r:02X}{g:02X}{b:02X}").into(),
                    }
                })
                .collect(),
        ),
        was_appearance: st.was(Target::Appearance).into(),
        was_gender: st.was(Target::Gender).into(),
        warn_appearance: warn(Target::Appearance),
        warn_gender: warn(Target::Gender),
    });
}

/// "#e9d6cc", "e9d6cc" -> RGB.
fn parse_rgb(t: &str) -> Option<[u8; 3]> {
    let h = t.trim().trim_start_matches('#');
    let v = (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok()).flatten()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// The card's best time per Arena quest, with the sets to pick from and the grade times
/// (the Quests page's Arena records tab).
pub(super) fn arena_rows(st: &State) -> Vec<ArenaRow> {
    let (s, base) = (st.save(), st.base());
    arena::quests()
        .iter()
        .enumerate()
        .map(|(q, a)| {
            let e = arena::best(s, base, q);
            let t = e.map_or(0, |e| arena::hundredths(e.time));
            let set_name = |&w: &u8| -> SharedString {
                let name = if a.prowler { palico::BIASES.get(w as usize) } else { character::USE_WEAPONS.get(w as usize) };
                tr(name.copied().unwrap_or("?")).into()
            };
            let grades = a.grades.iter().enumerate().map(|(g, &secs)| format!("{} ≤ {}:{:02}", targets::arena_grade(g as u8), secs / 60, secs % 60)).collect::<Vec<_>>();
            ArenaRow {
                index: q as i32,
                quest: targets::arena_quest(q).into(),
                has: e.is_some(),
                minutes: (t / 6000) as i32,
                seconds: (t / 100 % 60) as i32,
                hundredths: (t % 100) as i32,
                set: e.map_or(0, |e| e.set as i32),
                sets: model(a.sets.iter().map(set_name).collect()),
                grade: e.map_or("", |e| targets::arena_grade(e.grade)).into(),
                grades: grades.join(" · ").into(),
                was: st.was(Target::Arena(q)).into(),
                warning: e.and_then(|e| crate::warnings::arena_best(q, &e)).unwrap_or_default().into(),
            }
        })
        .collect()
}

/// The Guild Card section: the choices of each list (locked ones marked) and the card's
/// title, scene and pose.
fn guild_card(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let (s, base) = (st.save(), st.base());
    let n = assets::names();
    let list = |m: gc::Map, names: &[String]| {
        let (_, _, count) = m.at();
        strings((0..count).map(|i| {
            let name = targets::card_name(names, i);
            // the linking word 0 is "none", always available
            if (m == gc::Map::Links && i == 0) || gc::unlocked(s, base, m, i) { name } else { trf("{} · locked", &[&name]) }
        }))
    };
    api.set_gc_words(list(gc::Map::Words, &n.gc_words));
    api.set_gc_links(list(gc::Map::Links, &n.gc_links));
    api.set_gc_scenes(list(gc::Map::Scenes, &n.gc_scenes));
    api.set_gc_poses(list(gc::Map::Poses, &n.gc_poses));
    let [w1, link, w2] = gc::title(s, base);
    let warn = |t: Target| -> SharedString { crate::warnings::of(t, s, st.slot).unwrap_or_default().into() };
    api.set_card(CardInfo {
        word1: w1 as i32,
        link: link as i32,
        word2: w2 as i32,
        scene: gc::scene(s, base) as i32,
        pose: gc::pose(s, base) as i32,
        title: Target::Title.read(s, st.slot).into(),
        greeting: character::greeting(s, base).into(),
        was_greeting: st.was(Target::Greeting).into(),
        was_title: st.was(Target::Title).into(),
        was_scene: st.was(Target::Scene).into(),
        was_pose: st.was(Target::Pose).into(),
        warn_title: warn(Target::Title),
        warn_scene: warn(Target::Scene),
        warn_pose: warn(Target::Pose),
    });
}

/// The Guild Card weapon usage table in the card's order; the main weapon is the
/// largest total (the first one on a tie, as drawn).
pub(super) fn weapon_use_rows(st: &State) -> Vec<WeaponUseRow> {
    let (s, base) = (st.save(), st.base());
    let total = |w: usize| (0..3).map(|v| character::weapon_use(s, base, v, w) as i32).sum::<i32>();
    let top = character::USE_SHOWN.iter().map(|&w| total(w)).max().unwrap_or(0);
    let main = character::USE_SHOWN.iter().copied().find(|&w| top > 0 && total(w) == top);
    character::USE_SHOWN
        .iter()
        .map(|&w| WeaponUseRow {
            index: w as i32,
            name: tr(character::USE_WEAPONS[w]).into(),
            counts: model((0..3).map(|v| character::weapon_use(s, base, v, w) as i32).collect()),
            was: model((0..3).map(|v| SharedString::from(st.was(Target::WeaponUse(v, w)))).collect()),
            total: total(w),
            main: main == Some(w),
        })
        .collect()
}

pub(super) fn wire_character(ui: &AppWindow, st: &Shared) {
    // character: one value per edit, titled with its name
    on!(ui, st, on_set_character, |ui, s, key: SharedString, v: i32| {
        let v = v.max(0) as u32;
        let k = key.as_str();
        let t = match k {
            "hr" => Target::Hr,
            "hr-points" => Target::HrPoints,
            "funds" => Target::Funds,
            "wycademy" => Target::Wycademy,
            "village-star" => Target::VillageStar,
            "hub-star" => Target::HubStar,
            "play-h" | "play-m" => Target::Playtime,
            // "use:<venue>:<weapon>"
            _ if k.starts_with("style:") => match k[6..].parse::<usize>() {
                Ok(i) if i < character::STYLES.len() => Target::StyleUse(i),
                _ => return,
            },
            _ if k.starts_with("use:") => {
                let mut p = k[4..].split(':').map(|x| x.parse::<usize>().ok());
                match (p.next().flatten(), p.next().flatten()) {
                    (Some(v), Some(w)) if v < 3 && w < 15 => Target::WeaponUse(v, w),
                    _ => return,
                }
            }
            _ => match k.trim_start_matches(|c: char| c.is_alphabetic()).parse::<usize>() {
                Ok(i) if i < 4 => Target::Points(i, k.starts_with('g')),
                _ => return,
            },
        };
        let mut msg = None;
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            match k {
                "hr" => {
                    if !character::set_hr(sv, base, v as u16) {
                        msg = Some(tr("HR below 13 follows the Hub star level: edit Hub ★ instead"));
                    }
                }
                "hr-points" => character::set_hr_points(sv, base, v),
                "funds" => character::set_funds(sv, base, v),
                "wycademy" => character::set_wycademy(sv, base, v),
                "village-star" => sv.set_u16(base + mhgu_save::progress::VIL_STAR, (v as u16).clamp(1, 10)),
                "hub-star" => character::set_hub_star(sv, base, v as u16),
                "play-h" | "play-m" => {
                    let cur = character::get(sv, base).playtime;
                    let (h, m) = (cur / 3600, cur / 60 % 60);
                    let secs = if k == "play-h" { v * 3600 + m * 60 } else { h * 3600 + v.min(59) * 60 };
                    character::set_playtime(sv, base, secs + cur % 60);
                }
                _ => match t {
                    Target::Points(i, g) => character::set_village_points(sv, base, i, g, v),
                    Target::WeaponUse(venue, w) => character::set_weapon_use(sv, base, venue, w, v.min(u16::MAX as u32) as u16),
                    Target::CardQuests(i) => character::set_card_quests(sv, base, i, v.min(u16::MAX as u32) as u16),
                    Target::StyleUse(i) => character::set_style_use(sv, base, i, v.min(u16::MAX as u32) as u16),
                    _ => {}
                },
            }
            vec![]
        });
        if let Some(m) = msg {
            toast(&ui, m, true);
        }
    });
    // the card editor's fields; any entry can be picked, a locked one with a warning
    on!(ui, st, on_set_card, |ui, s, field: SharedString, v: i32| {
        let Ok(v) = u16::try_from(v) else { return };
        let f = field.as_str();
        let t = match f {
            "scene" => Target::Scene,
            "pose" => Target::Pose,
            _ => Target::Title,
        };
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            let mut tl = gc::title(sv, base);
            match f {
                "word1" => tl[0] = v,
                "link" => tl[1] = v,
                "word2" => tl[2] = v,
                "scene" => gc::set_scene(sv, base, v as u8),
                "pose" => gc::set_pose(sv, base, v as u8),
                _ => {}
            }
            gc::set_title(sv, base, tl);
            vec![]
        });
        let _ = &ui;
    });
    // a creation choice of the hunter: "gender", or face / hair / features / voice / clothing
    on!(ui, st, on_set_look, |ui, s, key: SharedString, v: i32| {
        let Ok(v) = u8::try_from(v) else { return };
        let (t, k) = match key.as_str() {
            "gender" => (Target::Gender, character::LOOK_GENDER),
            "face" => (Target::Appearance, character::LOOK_FACE),
            "hair" => (Target::Appearance, character::LOOK_HAIR),
            "features" => (Target::Appearance, character::LOOK_FEATURES),
            "voice" => (Target::Appearance, character::LOOK_VOICE),
            "clothing" => (Target::Appearance, character::LOOK_CLOTHING),
            _ => return,
        };
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            character::set_look(sv, base, k, v);
            vec![]
        });
        let _ = &ui;
    });
    on!(ui, st, on_set_look_colour, |ui, s, key: SharedString, hex: SharedString| {
        let Some(&(_, c)) = LOOK_COLOURS.iter().find(|x| x.0 == key.as_str()) else { return };
        let Some(rgb) = parse_rgb(&hex) else {
            return toast(&ui, tr("A colour is six hex digits, like #E9D6CC"), true);
        };
        let title = Target::Appearance.label(s.save(), s.slot);
        s.edit(Edit::one(Target::Appearance, title, Conf::Confirmed), |sv, base| {
            character::set_look_colour(sv, base, c, rgb);
            vec![]
        });
    });
    // an Arena best time (in 1/100 s, as shown) as a solo clear with the set picked; the
    // grade follows the time
    on!(ui, st, on_set_arena, |ui, s, q: i32, set: i32, time: i32| {
        let Some(q) = usize::try_from(q).ok().filter(|&q| q < arena::quests().len()) else { return };
        let t = Target::Arena(q);
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            arena::set_best(sv, base, q, arena::frames(time.max(1) as u32), set.clamp(0, 4) as u8);
            vec![]
        });
        let _ = &ui;
    });
    on!(ui, st, on_clear_arena, |ui, s, q: i32| {
        let Some(q) = usize::try_from(q).ok().filter(|&q| q < arena::quests().len()) else { return };
        let t = Target::Arena(q);
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
            arena::clear(sv, base, q);
            vec![]
        });
        let _ = &ui;
    });
    on!(ui, st, on_set_greeting, |ui, s, text: SharedString| {
        s.edit(Edit::one(Target::Greeting, tr("Guild Card greeting").into(), Conf::Confirmed), |sv, base| {
            character::set_greeting(sv, base, &text);
            vec![]
        });
        let _ = &ui;
    });
    on!(ui, st, on_set_name, |ui, s, name: SharedString| {
        s.edit(Edit::one(Target::Name, tr("Name").into(), Conf::Confirmed).note(tr("Written to the save, the player record and the Guild Card")), |sv, base| {
            character::set_name(sv, base, &name);
            vec![]
        });
        let _ = &ui;
    });
}
