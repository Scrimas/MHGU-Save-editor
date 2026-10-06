//! The Character page: its models and the callbacks that edit it.

use super::*;

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
        gender: match c.gender {
            0 => "Type 1 (male)".into(),
            1 => "Type 2 (female)".into(),
            g => format!("{g}").into(),
        },
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
            name: character::USE_WEAPONS[w].into(),
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
                        msg = Some("HR below 13 follows the Hub star level: edit Hub ★ instead");
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
                    _ => {}
                },
            }
            vec![]
        });
        if let Some(m) = msg {
            toast(&ui, m, true);
        }
    });
    on!(ui, st, on_set_name, |ui, s, name: SharedString| {
        s.edit(Edit::one(Target::Name, "Name".into(), Conf::Confirmed).note("Written to the save, the player record and the Guild Card"), |sv, base| {
            character::set_name(sv, base, &name);
            vec![]
        });
        let _ = &ui;
    });
}
