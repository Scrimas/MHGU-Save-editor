//! My Sets and Palico equipment sets (the Equipment page's last two tabs), and the
//! hunting style, Hunter Arts and armor pigment the hunter has on (Character page).

use super::*;
use crate::targets::{SetPart, SET_PARTS};
use mhgu_save::sets::{self, Arts, Pigment};

/// Which sets the Equipment page shows: My Sets (false) or Palico sets (true), none on
/// the box tabs.
pub(super) fn sets_tab(ui: &AppWindow) -> Option<bool> {
    match ui.global::<Api>().get_equip_owner() {
        2 => Some(false),
        3 => Some(true),
        _ => None,
    }
}

/// Place `j` of a set: weapon, head … legs, talisman; Palico weapon, head, body.
pub fn set_piece_label(j: usize) -> &'static str {
    match j {
        0 => tr("Weapon"),
        6 => tr("Talisman"),
        _ => armor_parts().get(j.wrapping_sub(1)).copied().unwrap_or("?"),
    }
}

pub fn palico_piece_label(j: usize) -> &'static str {
    [tr("Palico weapon"), tr("Palico head"), tr("Palico body")].get(j).copied().unwrap_or("?")
}

/// Whether a box entry of kind `k` goes in place `j`.
fn fits(palico: bool, j: usize, k: Kind) -> bool {
    match (palico, j) {
        (true, _) => k == Kind::Other(22 + j as u8),
        (false, 0) => matches!(k, Kind::Weapon(_)),
        (false, 6) => k == Kind::Talisman,
        (false, _) => k.code() as usize == j,
    }
}

/// The piece in box entry `g` as Review shows a set's gear.
pub fn box_piece_value(s: &mhgu_save::Save, base: usize, owner: Owner, g: u16) -> String {
    if g as usize >= owner.len() {
        return format!("#{g}");
    }
    equip_name(owner, &equipment::get(s, base, owner, g as usize))
}

/// The pieces place `j` can take: "None", then each fitting box entry (and the one the
/// set holds, whatever it is), with the box indices.
fn piece_choices(s: &mhgu_save::Save, base: usize, palico: bool, j: usize, held: u16) -> (Vec<String>, Vec<Option<usize>>) {
    let owner = if palico { Owner::Palico } else { Owner::Hunter };
    let mut labels = vec![tr("None").to_string()];
    let mut ids = vec![None];
    for i in 0..owner.len() {
        let e = equipment::get(s, base, owner, i);
        if fits(palico, j, e.kind()) || i == held as usize {
            let name = if e.kind() == Kind::Talisman { talisman_skills(&e) } else { equip_name(owner, &e) };
            labels.push(trf("{} · box slot {}", &[&name, &(i + 1)]));
            ids.push(Some(i));
        }
    }
    (labels, ids)
}

/// "#FF711E, own colour, …" in the parts' order.
pub fn pigment_value(p: &Pigment) -> String {
    (0..5)
        .map(|k| {
            let [r, g, b, _] = p.rgba[k];
            if p.own[k] { tr("own colour").to_string() } else { format!("#{r:02X}{g:02X}{b:02X}") }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// "Valor Style: Energy Blade I (SP)", "Unused".
pub fn arts_value(a: &Arts) -> String {
    let Some(style) = character::STYLES.get(a.style as usize) else { return tr("Unused").into() };
    let v: Vec<String> = (0..3)
        .filter(|&k| a.ids[k] != 0)
        .map(|k| {
            let n = crate::warnings::art_name(a.ids[k]);
            if a.sp >> k & 1 != 0 { trf("{} (SP)", &[&n]) } else { n }
        })
        .collect();
    format!("{}: {}", tr(style), if v.is_empty() { tr("no arts").to_string() } else { v.join(", ") })
}

/// The arts weapon class `w` takes (all of them without one), the ones `held` too: their
/// labels after "None", and the IDs.
fn art_choices(s: &mhgu_save::Save, base: usize, w: Option<u8>, held: [u8; 3]) -> (Vec<String>, Vec<u8>) {
    let mut labels = vec![tr("None").to_string()];
    let mut ids = vec![0u8];
    for &(id, ref name) in &tables().arts {
        let id = id as u8;
        let takes = match sets::art_class(id) {
            Some(sets::ArtFor::Any) => true,
            Some(sets::ArtFor::Weapon(c)) => w.is_none_or(|w| w == c),
            None => false,
        };
        if takes || held.contains(&id) {
            let on = s.bit(base + mhgu_save::progress::ARTS, id as usize);
            labels.push(if on { name.clone() } else { trf("{} · locked", &[name]) });
            ids.push(id);
        }
    }
    (labels, ids)
}

fn arts_info(s: &mhgu_save::Save, base: usize, a: &Arts, w: Option<u8>, was: String, warning: String) -> ArtsInfo {
    let (labels, ids) = art_choices(s, base, w, a.ids);
    ArtsInfo {
        style: a.style as i32,
        slots: a.slots() as i32,
        picks: model((0..3).map(|k| ids.iter().position(|&x| x == a.ids[k]).unwrap_or(0) as i32).collect()),
        sp: model((0..3).map(|k| a.sp >> k & 1 != 0).collect()),
        choices: strings(labels),
        weapon: w.and_then(|w| weapon_classes().get(w as usize).copied()).unwrap_or("").into(),
        was: was.into(),
        warning: warning.into(),
    }
}

fn pigment_rows(p: &Pigment) -> Vec<PigmentRow> {
    (0..5)
        .map(|k| {
            let [r, g, b, _] = p.rgba[k];
            PigmentRow { part: tr(sets::PIGMENT_PARTS[k]).into(), colour: slint::Color::from_rgb_u8(r, g, b), hex: format!("#{r:02X}{g:02X}{b:02X}").into(), own: p.own[k] }
        })
        .collect()
}

/// The weapon class of a set's weapon (none without one).
fn set_weapon(s: &mhgu_save::Save, base: usize, m: &sets::MySet) -> Option<u8> {
    let g = m.gear[0] as usize;
    match g < equipment::BOX_N {
        true => match equipment::get(s, base, Owner::Hunter, g).kind() {
            Kind::Weapon(c) => Some(c),
            _ => None,
        },
        false => None,
    }
}

/// The Character page's style, arts and pigment.
pub(super) fn char_loadout(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let (s, base) = (st.save(), st.base());
    let warn = crate::warnings::of(Target::Arts, s, st.slot).unwrap_or_default();
    api.set_char_arts(arts_info(s, base, &sets::arts(s, base), Some(sets::weapon_class(s, base)), st.was(Target::Arts), warn));
    api.set_char_pigment(model(pigment_rows(&sets::pigment(s, base))));
    api.set_was_pigment(st.was(Target::Pigment).into());
}

fn sel(palico: bool) -> usize {
    view(|v| if palico { v.palset_sel } else { v.set_sel }).max(0) as usize
}

/// The set list (unused runs collapsed, like the item loadouts) and the set selected.
pub(super) fn sets_page(ui: &AppWindow, st: &State, palico: bool) {
    let api = ui.global::<Api>();
    let (s, base) = (st.save(), st.base());
    let n = if palico { sets::PALICO_SETS_N } else { sets::MY_SETS_N };
    let (at, size) = if palico { (sets::PALICO_SETS, sets::PALICO_SET) } else { (sets::MY_SETS, sets::MY_SET) };
    let mut rows: Vec<SetRow> = vec![];
    let mut run: Option<(usize, usize)> = None;
    let flush = |run: &mut Option<(usize, usize)>, rows: &mut Vec<SetRow>| {
        if let Some((a, b)) = run.take() {
            let name = if a == b { trf("Set {} · unused", &[&(a + 1)]) } else { trf("Sets {}–{} · unused", &[&(a + 1), &(b + 1)]) };
            rows.push(SetRow { index: -1, first: a as i32, last: b as i32, name: name.into(), sub: "".into(), used: false, changed: false });
        }
    };
    let mut used = 0;
    for k in 0..n {
        let changed = st.changed(base + at + size * k, size);
        let (name, sub, on) = if palico {
            let p = sets::palico_set(s, base, k);
            let v: Vec<String> = p.gear.iter().filter(|&&g| g != sets::NO_BOX).map(|&g| box_piece_value(s, base, Owner::Palico, g)).collect();
            (p.name.clone(), v.join(", "), p.used())
        } else {
            let m = sets::my_set(s, base, k);
            let weapon = set_weapon(s, base, &m).and_then(|w| weapon_classes().get(w as usize).copied());
            let style = character::STYLES.get(m.arts.style as usize).map(|x| tr(x));
            (m.name.clone(), [weapon, style].into_iter().flatten().collect::<Vec<_>>().join(" · "), m.used())
        };
        if !on && !changed {
            run = Some(run.map_or((k, k), |(a, _)| (a, k)));
            continue;
        }
        used += on as usize;
        flush(&mut run, &mut rows);
        rows.push(SetRow { index: k as i32, first: k as i32, last: k as i32, name: name.into(), sub: sub.into(), used: on, changed });
    }
    flush(&mut run, &mut rows);
    api.set_set_rows(keep(api.get_set_rows(), rows));
    api.set_equip_summary(
        if palico { trf("{} of {} Palico sets used · Derived: not checked in game yet", &[&used, &n]) } else { trf("{} of {} My Sets used · Derived except the pigment", &[&used, &n]) }
            .into(),
    );
    let k = sel(palico).min(n - 1);
    let d = if palico {
        let p = sets::palico_set(s, base, k);
        let t = Target::PalicoSet(k);
        SetDetail {
            index: k as i32,
            used: p.used(),
            palico: true,
            name: p.name.clone().into(),
            pieces: model(
                (0..3)
                    .map(|j| {
                        let (labels, ids) = piece_choices(s, base, true, j, p.gear[j]);
                        let pick = ids.iter().position(|&x| x == Some(p.gear[j] as usize)).unwrap_or(0);
                        SetPiece { part: palico_piece_label(j).into(), pick: pick as i32, choices: strings(labels) }
                    })
                    .collect(),
            ),
            was_name: st.was(t).into(),
            warn_gear: crate::warnings::of(t, s, st.slot).unwrap_or_default().into(),
            ..Default::default()
        }
    } else {
        let m = sets::my_set(s, base, k);
        let was = |p: SetPart| -> SharedString { st.was(Target::MySet(k, p)).into() };
        let arts_warn = crate::warnings::of(Target::MySet(k, SetPart::Arts), s, st.slot).unwrap_or_default();
        SetDetail {
            index: k as i32,
            used: m.used(),
            palico: false,
            name: m.name.clone().into(),
            pieces: model(
                (0..sets::PIECES)
                    .map(|j| {
                        let (labels, ids) = piece_choices(s, base, false, j, m.gear[j]);
                        let pick = ids.iter().position(|&x| x == Some(m.gear[j] as usize)).unwrap_or(0);
                        SetPiece { part: set_piece_label(j).into(), pick: pick as i32, choices: strings(labels) }
                    })
                    .collect(),
            ),
            pigment: model(pigment_rows(&m.pigment)),
            arts: arts_info(s, base, &m.arts, set_weapon(s, base, &m), was(SetPart::Arts).into(), arts_warn),
            was_name: was(SetPart::Name),
            was_gear: was(SetPart::Gear),
            was_pigment: was(SetPart::Pigment),
            warn_gear: crate::warnings::of(Target::MySet(k, SetPart::Gear), s, st.slot).unwrap_or_default().into(),
        }
    };
    api.set_set_detail(d);
}

/// Stage `f` as one edit of `t` (edits of the same value merge); says so when Confirmed
/// only refuses it.
fn stage(ui: &AppWindow, s: &mut State, t: Target, conf: Conf, f: impl FnOnce(&mut mhgu_save::Save, usize)) {
    if refused(ui, conf) {
        return;
    }
    let title = t.label(s.save(), s.slot);
    s.edit(Edit::one(t, title, conf), |sv, base| {
        f(sv, base);
        vec![]
    });
}

pub(super) fn wire_sets(ui: &AppWindow, st: &Shared) {
    on!(ui, st, on_select_set, |ui, s, k: i32| {
        let palico = sets_tab(&ui) == Some(true);
        view(|v| if palico { v.palset_sel = k } else { v.set_sel = k });
        let _ = &s;
    });
    on!(ui, st, on_set_set_name, |ui, s, name: SharedString| {
        let Some(palico) = sets_tab(&ui) else { return };
        let k = sel(palico);
        if palico {
            stage(&ui, &mut s, Target::PalicoSet(k), Conf::Derived, |sv, base| sets::set_palico_set_name(sv, base, k, &name));
        } else if sets::my_set(s.save(), s.base(), k).used() {
            stage(&ui, &mut s, Target::MySet(k, SetPart::Name), Conf::Derived, |sv, base| sets::set_my_set_name(sv, base, k, &name));
        }
    });
    on!(ui, st, on_set_set_piece, |ui, s, j: i32, pick: i32| {
        let Some(palico) = sets_tab(&ui) else { return };
        let k = sel(palico);
        let (sv, base) = (s.save(), s.base());
        let Ok(j) = usize::try_from(j) else { return };
        if (palico && j >= 3) || j >= sets::PIECES {
            return;
        }
        let held = if palico { sets::palico_set(sv, base, k).gear[j] } else { sets::my_set(sv, base, k).gear[j] };
        let (_, ids) = piece_choices(sv, base, palico, j, held);
        let Some(&i) = usize::try_from(pick).ok().and_then(|p| ids.get(p)) else { return };
        if palico {
            stage(&ui, &mut s, Target::PalicoSet(k), Conf::Derived, |sv, base| sets::set_palico_set_piece(sv, base, k, j, i));
        } else if sets::my_set(sv, base, k).used() {
            stage(&ui, &mut s, Target::MySet(k, SetPart::Gear), Conf::Derived, |sv, base| sets::set_my_set_piece(sv, base, k, j, i));
        }
    });
    // the game's "register": the selected My Set becomes what the hunter has on
    on!(ui, st, on_set_from_current, |ui, s| {
        if sets_tab(&ui) != Some(false) || refused(&ui, Conf::Derived) {
            return;
        }
        let k = sel(false);
        let targets = SET_PARTS.iter().map(|&p| Target::MySet(k, p)).collect();
        let e = Edit { key: String::new(), title: trf("My Set {}: what you have on", &[&(k + 1)]), detail: tr("Equipment").into(), note: String::new(), conf: Conf::Derived, targets };
        s.edit(e, |sv, base| {
            sets::save_current(sv, base, k);
            vec![]
        });
    });
    on!(ui, st, on_clear_set, |ui, s| {
        let Some(palico) = sets_tab(&ui) else { return };
        if refused(&ui, Conf::Derived) {
            return;
        }
        let k = sel(palico);
        let (title, targets) = if palico {
            (trf("Palico set {} cleared", &[&(k + 1)]), vec![Target::PalicoSet(k)])
        } else {
            (trf("My Set {} cleared", &[&(k + 1)]), SET_PARTS.iter().map(|&p| Target::MySet(k, p)).collect())
        };
        let e = Edit { key: String::new(), title, detail: tr("Equipment").into(), note: String::new(), conf: Conf::Derived, targets };
        s.edit(e, |sv, base| {
            if palico { sets::clear_palico_set(sv, base, k) } else { sets::clear_my_set(sv, base, k) }
            vec![]
        });
    });
    // style, arts and SP of what the hunter has on (where < 0) or of the selected My Set
    on!(ui, st, on_set_arts, |ui, s, at: i32, field: SharedString, slot: i32, v: i32| {
        let (sv, base) = (s.save(), s.base());
        let k = sel(false);
        let (cur, weapon, t) = if at < 0 {
            (sets::arts(sv, base), Some(sets::weapon_class(sv, base)), Target::Arts)
        } else {
            let m = sets::my_set(sv, base, k);
            if !m.used() {
                return;
            }
            (m.arts, set_weapon(sv, base, &m), Target::MySet(k, SetPart::Arts))
        };
        let mut a = cur;
        let slot = slot.clamp(0, 2) as usize;
        match field.as_str() {
            "style" if (0..6).contains(&v) => a.set_style(v as u8),
            "art" => {
                let (_, ids) = art_choices(sv, base, weapon, cur.ids);
                let Some(&id) = usize::try_from(v).ok().and_then(|p| ids.get(p)) else { return };
                a.set_art(slot, id);
            }
            "sp" => a.set_sp(slot, v != 0),
            _ => return,
        }
        stage(&ui, &mut s, t, Conf::Derived, |sv, base| if at < 0 { sets::set_arts(sv, base, a) } else { sets::set_my_set_arts(sv, base, k, a) });
    });
    // a My Set's colours were checked in game; what the hunter has on not yet
    on!(ui, st, on_set_pigment, |ui, s, at: i32, part: i32, hex: SharedString| {
        let Some(p) = usize::try_from(part).ok().filter(|&p| p < 5) else { return };
        let Some(rgb) = parse_rgb(&hex) else {
            return toast(&ui, tr("A colour is six hex digits, like #E9D6CC"), true);
        };
        let k = sel(false);
        if at < 0 {
            stage(&ui, &mut s, Target::Pigment, Conf::Derived, |sv, base| sets::set_colour(sv, base, p, rgb));
        } else if sets::my_set(s.save(), s.base(), k).used() {
            stage(&ui, &mut s, Target::MySet(k, SetPart::Pigment), Conf::Confirmed, |sv, base| sets::set_my_set_colour(sv, base, k, p, rgb));
        }
    });
    on!(ui, st, on_own_pigment, |ui, s, at: i32, part: i32| {
        let Some(p) = usize::try_from(part).ok().filter(|&p| p < 5) else { return };
        let k = sel(false);
        if at < 0 {
            stage(&ui, &mut s, Target::Pigment, Conf::Derived, |sv, base| sets::set_own_colour(sv, base, p));
        } else if sets::my_set(s.save(), s.base(), k).used() {
            stage(&ui, &mut s, Target::MySet(k, SetPart::Pigment), Conf::Derived, |sv, base| sets::set_my_set_own_colour(sv, base, k, p));
        }
    });
}
