//! The Palicoes page: its models and the callbacks that edit it. Tabs: the character's
//! Palicoes and those for hire (one editor), the roles, counters, Dojo and teams, the
//! StreetPass Palicoes and the scouting request (`mhgu_save::otomo`, all Derived).

use super::*;
use crate::{CardRow, InfoRow, OptionRow};
use mhgu_save::otomo;

/// The roles' names (`otomo::ROLES`).
pub fn role_name(r: usize) -> String {
    match r {
        0 => tr("Palico played as Prowler"),
        1 => tr("Hunting buddy 1"),
        _ => tr("Hunting buddy 2"),
    }
    .into()
}

/// A scouting request field's name (`otomo::REQUEST_FIELDS`).
pub fn request_field_name(k: usize) -> String {
    match otomo::REQUEST_FIELDS[k].0 {
        "mode" => tr("Scout Method"),
        "forte" => tr("Forte"),
        "target" => tr("Target"),
        key => look_label(key),
    }
    .into()
}

/// Its choices by stored value: 0 is none or any.
fn request_choices(k: usize) -> Vec<String> {
    let any = tr("Any").to_string();
    match otomo::REQUEST_FIELDS[k].0 {
        "mode" => [tr("None"), tr("Ability"), tr("Looks")].map(String::from).to_vec(),
        "forte" => std::iter::once(any).chain(palico::BIASES.iter().map(|&b| tr(b).to_string())).collect(),
        "target" => std::iter::once(any).chain(palico::TARGETS[1..].iter().map(|&t| tr(t).to_string())).collect(),
        key => {
            let coats = &assets::names().palico_coats;
            std::iter::once(any)
                .chain((0..otomo::request_max(k) as usize).map(|c| match coats.get(c) {
                    Some(n) if key == "coat" => n.clone(),
                    _ => trf("Type {}", &[&(c + 1)]),
                }))
                .collect()
        }
    }
}

pub fn request_value_name(k: usize, v: u8) -> String {
    request_choices(k).get(v as usize).cloned().unwrap_or_else(|| v.to_string())
}

/// The character's Palicoes in place order, the roles' pickers' entries after "None".
fn own_palicoes(s: &mhgu_save::Save, base: usize) -> Vec<usize> {
    (0..palico::LIST_N).filter(|&i| !palico::is_empty(s, base, i)).collect()
}

/// A Palico's edits: Confirmed, Derived for one for hire (hired as edited, not checked).
fn pal_conf(i: usize) -> Conf {
    if palico::for_hire(i) { Conf::Derived } else { Conf::Confirmed }
}

/// The Team and Dojo, StreetPass and Scouting tabs.
fn other_tabs(ui: &AppWindow, st: &State, tab: i32) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let was = |t: Target| SharedString::from(st.was(t));
    let name = |i: usize| palico::get(s, base, i).name;
    let names = |v: &[usize]| if v.is_empty() { tr("None").to_string() } else { v.iter().map(|&i| name(i)).collect::<Vec<_>>().join(", ") };
    let line = |head: String, n: String, value: String| InfoRow { head: head.into(), name: n.into(), value: value.into() };
    let mut fields: Vec<OptionRow> = vec![];
    let mut info: Vec<InfoRow> = vec![];
    let mut mail: Vec<CardRow> = vec![];
    match tab {
        2 => {
            let own = own_palicoes(s, base);
            let choices: Vec<String> = std::iter::once(tr("None").to_string()).chain(own.iter().map(|&i| name(i))).collect();
            for r in 0..otomo::ROLE_N {
                let t = Target::PalRole(r);
                let at = otomo::role(s, base, r).and_then(|i| own.iter().position(|&x| x == i)).map_or(0, |k| k as i32 + 1);
                let head = if r == 0 { tr("Roles") } else { "" };
                fields.push(OptionRow { id: t.key().into(), head: head.into(), name: role_name(r).into(), choices: strings(choices.clone()), value: at, was: was(t), ..Default::default() });
            }
            for (t, n, v, max) in [
                (Target::DojoDone, tr("Palico Dojo sessions completed"), otomo::dojo_done(s, base), otomo::DOJO_MAX),
                (Target::PalicoesHired, tr("Palicoes hired"), otomo::hired(s, base), otomo::HIRED_MAX),
            ] {
                let head = if t == Target::DojoDone { tr("Counters") } else { "" };
                fields.push(OptionRow { id: t.key().into(), head: head.into(), name: n.into(), value: v as i32, number: true, max: max as i32, was: was(t), ..Default::default() });
            }
            let move_or_skill = |skill: bool, id: u8| {
                let n = &assets::names();
                if skill { targets::palico_name(&n.palico_skills, id) } else { targets::palico_name(&n.support_moves, id) }
            };
            let training = otomo::training(s, base);
            if training.is_empty() {
                info.push(line(tr("Palico Dojo").into(), tr("Training").into(), tr("None").into()));
            }
            for (k, t) in training.iter().enumerate() {
                let mut v = trf("{} · {} of {} sessions left", &[&name(t.palico), &t.left, &t.booked]);
                if let Some((skill, id)) = t.learns {
                    v = format!("{v} · {}", trf("learning {}", &[&move_or_skill(skill, id)]));
                }
                info.push(line(if k == 0 { tr("Palico Dojo").into() } else { String::new() }, trf("Training {}", &[&(k + 1)]), v));
            }
            let teaching = match otomo::teaching(s, base) {
                Some(t) => trf("{} teaches {} to {}", &[&name(t.teacher), &move_or_skill(t.skill, t.id), &names(&t.students)]),
                None => tr("None").into(),
            };
            info.push(line(String::new(), tr("Teaching session").into(), teaching));
            info.push(line(tr("Palico Board").into(), tr("Team").into(), names(&otomo::board(s, base))));
            let state = match otomo::expedition(s, base) {
                0 => tr("At home"),
                1 => tr("Out on an expedition"),
                _ => tr("Back, results waiting"),
            };
            info.push(line(tr("Meownster Hunters").into(), tr("Expedition").into(), state.into()));
            info.push(line(String::new(), tr("Team").into(), names(&otomo::members(s, base))));
        }
        3 => {
            let row = |slot: i32, tag: &str, p: &otomo::SpPalico| CardRow {
                slot,
                name: p.name.clone().into(),
                tag: tag.into(),
                line1: trf("Lv {} · {}", &[&p.level, &palico::BIASES.get(p.bias as usize).map_or("?", |&b| tr(b))]).into(),
                line2: [(!p.sender.is_empty()).then(|| trf("From {}", &[&p.sender])), (!p.owner.is_empty()).then(|| trf("Original owner {}", &[&p.owner]))]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ")
                    .into(),
                greeting: p.greeting.clone().into(),
            };
            mail.extend(otomo::to_send(s, base).map(|p| row(-1, tr("To send"), &p)));
            mail.extend(otomo::inbox(s, base).iter().map(|(k, p)| row(*k as i32, tr("Inbox"), p)));
        }
        4 => {
            for k in 0..otomo::REQUEST_FIELDS.len() {
                let t = Target::Scouting(k);
                let choices = request_choices(k);
                let v = otomo::request(s, base, k) as i32;
                let head = match k {
                    0 => tr("Scouting Conditions"),
                    1 => tr("Ability"),
                    3 => tr("Looks"),
                    _ => "",
                };
                fields.push(OptionRow {
                    id: t.key().into(),
                    head: head.into(),
                    name: request_field_name(k).into(),
                    value: if (0..choices.len() as i32).contains(&v) { v } else { -1 },
                    choices: strings(choices),
                    was: was(t),
                    ..Default::default()
                });
            }
        }
        _ => {}
    }
    api.set_palico_fields(model(fields));
    api.set_palico_info(model(info));
    api.set_palico_mail(model(mail));
}

/// A look's name in the interface language.
pub fn look_label(key: &str) -> &'static str {
    match key {
        "coat" => tr("Coat"),
        "coat_colour" => tr("Coat colour"),
        "clothing" => tr("Clothing"),
        "clothing_colour" => tr("Clothing colour"),
        "eyes" => tr("Eyes"),
        "eye_left" => tr("Left eye colour"),
        "eye_right" => tr("Right eye colour"),
        "ears" => tr("Ears"),
        "tail" => tr("Tail"),
        "voice" => tr("Voice"),
        _ => "?",
    }
}

/// The Palico's looks: a picker per choice (coats by name, the rest "Type n"), swatches
/// for the colours.
fn look_rows(p: &palico::Palico) -> Vec<LookRow> {
    let coats = &assets::names().palico_coats;
    palico::look_fields()
        .into_iter()
        .map(|f| {
            let rgb = f.rgb(p);
            let colour = |[r, g, b]: [u8; 3]| slint::Color::from_rgb_u8(r, g, b);
            LookRow {
                key: f.key.into(),
                label: look_label(f.key).into(),
                is_colour: f.colour.is_some(),
                index: if f.colour.is_some() { f.palette.iter().position(|&c| c == rgb).map_or(-1, |k| k as i32) } else { f.get(p) as i32 - f.first as i32 },
                choices: strings((0..f.choices).map(|k| match coats.get(k as usize) {
                    Some(n) if f.key == "coat" => n.clone(),
                    _ => trf("Type {}", &[&(k + 1)]),
                })),
                swatches: model(f.palette.iter().map(|&c| colour(c)).collect()),
                colour: colour(rgb),
            }
        })
        .collect()
}

pub(super) fn palico_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let q = view(|v| v.palico_search.to_lowercase());
    let all = own_palicoes(s, base).len();
    let tab = api.get_palico_tab();
    api.set_palico_tabs(strings([tr("My Palicoes"), tr("For hire"), tr("Team and Dojo"), tr("StreetPass"), tr("Palico Scout")].map(String::from)));
    api.set_palico_note(
        match tab {
            1 => tr("The Palicoes the game offers for hire: an edited one joins as edited when hired. The game draws a new list from time to time."),
            2 => tr("A Prowler fights with its Palico's support moves and skills (My Palicoes). The game gives award 53 from 50 Dojo sessions and title words from 10, 30, 50 and 80 hires at its next check. The Dojo and the teams are read-only."),
            3 => tr("Palicoes received by StreetPass wait in the inbox until hired or let go; the one to send goes out with the Guild Card."),
            4 => tr("What the Palico Scout looks for when scouting a Palico: by ability (forte, target) or by looks. The colours asked for are left as they are."),
            _ => "",
        }
        .into(),
    );
    other_tabs(ui, st, tab);
    let places = if tab == 1 { palico::LIST_N..palico::PLACES } else { 0..palico::LIST_N };
    let rows: Vec<PalicoRow> = places
        .clone()
        .filter(|&i| !palico::is_empty(s, base, i))
        .map(|i| (i, palico::get(s, base, i)))
        .filter(|(_, p)| q.is_empty() || p.name.to_lowercase().contains(&q))
        .map(|(i, p)| {
            PalicoRow {
                index: i as i32,
                name: p.name.into(),
                sub: trf("Lv {} · {}", &[&p.level, &palico::BIASES.get(p.bias as usize).map_or("?", |&b| tr(b))]).into(),
                changed: st.changed(base + palico::offset(i), palico::RECORD),
            }
        })
        .collect();
    let mut sel = view(|v| v.palico_sel);
    if !usize::try_from(sel).is_ok_and(|i| places.contains(&i)) {
        sel = rows.first().map(|r| r.index).unwrap_or(-1);
        view(|v| v.palico_sel = sel);
    }
    api.set_palico_summary(trn("{n} Palico", "{n} Palicoes", all as i64, &[]).into());
    api.set_palicoes(keep(api.get_palicoes(), rows));
    api.set_biases(strings(palico::BIASES.iter().map(|&s| tr(s).to_string())));
    api.set_palico_targets(strings(palico::TARGETS[1..].iter().map(|&s| tr(s).to_string())));
    let names = assets::names();
    let choices = |names: &[String], n: usize| strings((0..n).map(|id| targets::palico_name(names, id as u8)));
    api.set_palico_move_names(choices(&names.support_moves, palico::NO_MOVE as usize));
    api.set_palico_skill_names(choices(&names.palico_skills, palico::NO_SKILL as usize));
    // the list's slots: innate first, taught at the end, ticked when equipped
    let rows = |list: &[u8], on: &[u8], head: usize, taught: usize, names: &[String]| -> Vec<PalicoEntry> {
        list.iter()
            .enumerate()
            .map(|(k, &id)| PalicoEntry {
                slot: k as i32,
                id: id as i32,
                name: targets::palico_name(names, id).into(),
                innate: k < head,
                taught: k + taught >= list.len(),
                on: id != 0 && on.contains(&id),
            })
            .collect()
    };
    api.set_palico(if sel >= 0 {
        let i = sel as usize;
        let p = palico::get(s, base, i);
        let was = |f: targets::Pal| -> SharedString { st.was(Target::Palico(i, f)).into() };
        let warning: Vec<String> = [targets::Pal::Level, targets::Pal::Bias, targets::Pal::Target].into_iter().filter_map(|f| crate::warnings::palico(&p, f)).collect();
        let warn = |f: targets::Pal| -> SharedString { crate::warnings::palico(&p, f).unwrap_or_default().into() };
        let equipped = p.moves.iter().filter(|&&x| x != 0).count();
        PalicoDetail {
            move_rows: model(rows(p.move_list(), &p.moves, p.innate_moves(), p.taught_moves(), &names.support_moves)),
            skill_rows: model(rows(p.skill_list(), &p.skills_on, palico::INNATE_SKILLS, palico::TAUGHT_SKILLS, &names.palico_skills)),
            moves_used: trf("{} of {} equipped", &[&equipped, &p.move_slots()]).into(),
            skills_used: trf("{} of {} skill slots used", &[&p.skills_cost(), &p.skill_slots()]).into(),
            look_rows: model(look_rows(&p)),
            was_looks: was(targets::Pal::Looks),
            warn_looks: warn(targets::Pal::Looks),
            was_moves: was(targets::Pal::Moves),
            was_skills: was(targets::Pal::Skills),
            warn_moves: warn(targets::Pal::Moves),
            warn_skills: warn(targets::Pal::Skills),
            warning: warning.join("; ").into(),
            index: sel,
            name: p.name.into(),
            level: p.level as i32,
            exp: p.exp as i32,
            bias: p.bias as i32,
            target: p.target as i32,
            greeting: p.greeting.into(),
            owner: p.owner.into(),
            was_name: was(targets::Pal::Name),
            was_level: was(targets::Pal::Level),
            was_exp: was(targets::Pal::Exp),
            was_bias: was(targets::Pal::Bias),
            was_greeting: was(targets::Pal::Greeting),
            was_owner: was(targets::Pal::Owner),
            was_target: was(targets::Pal::Target),
        }
    } else {
        PalicoDetail { index: -1, ..Default::default() }
    });
}

pub(super) fn wire_palicoes(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    // palicoes
    on!(ui, st, on_filter_palicoes, |ui, s, q: SharedString| {
        view(|v| v.palico_search = q.to_string());
        let _ = (&ui, &s);
    });
    on!(ui, st, on_select_palico, |ui, s, i: i32| {
        view(|v| v.palico_sel = i);
        let _ = (&ui, &s);
    });
    on!(ui, st, on_select_palico_tab, |ui, s, i: i32| {
        let _ = &s;
        ui.global::<Api>().set_palico_tab(i.clamp(0, 4));
        // the tab's first Palico
        view(|v| v.palico_sel = -1);
    });
    // a role ("pal-role:r", choice 0 = none, else the k-th Palico), a counter or a
    // scouting field ("scouting:id")
    on!(ui, st, on_set_palico_field, |ui, s, id: SharedString, v: i32| {
        if refused(&ui, Conf::Derived) {
            return;
        }
        type Write = Box<dyn FnOnce(&mut mhgu_save::Save, usize) -> Vec<Target>>;
        let (t, f): (Target, Write) = if let Some(r) = id.strip_prefix("pal-role:").and_then(|r| r.parse::<usize>().ok()).filter(|&r| r < otomo::ROLE_N) {
            let p = match usize::try_from(v - 1) {
                Ok(k) => match own_palicoes(s.save(), s.base()).get(k) {
                    Some(&i) => Some(i),
                    None => return,
                },
                Err(_) => None,
            };
            // a role that held the Palico is left empty: listed with it
            (Target::PalRole(r), Box::new(move |sv, base| otomo::set_role(sv, base, r, p).into_iter().map(Target::PalRole).collect()))
        } else {
            let v = v.clamp(0, 255) as u8;
            match id.as_str() {
                "dojo-done" => (Target::DojoDone, Box::new(move |sv, base| {
                    otomo::set_dojo_done(sv, base, v);
                    vec![]
                })),
                "palicoes-hired" => (Target::PalicoesHired, Box::new(move |sv, base| {
                    otomo::set_hired(sv, base, v);
                    vec![]
                })),
                id => {
                    let Some(k) = id.strip_prefix("scouting:").and_then(|f| otomo::REQUEST_FIELDS.iter().position(|x| x.0 == f)) else { return };
                    (Target::Scouting(k), Box::new(move |sv, base| {
                        otomo::set_request(sv, base, k, v);
                        vec![]
                    }))
                }
            }
        };
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Derived), f);
    });
    on!(ui, st, on_remove_palico_mail, |ui, s, k: i32| {
        let Ok(k) = usize::try_from(k) else { return };
        let Some((_, p)) = otomo::inbox(s.save(), s.base()).into_iter().find(|x| x.0 == k) else { return };
        if refused(&ui, Conf::Derived) {
            return;
        }
        let e = Edit::one(Target::PalInbox, trf("Remove the StreetPass Palico {}", &[&p.name]), Conf::Derived);
        s.edit(e, |sv, base| {
            otomo::remove_inbox(sv, base, k);
            vec![]
        });
    });
    on!(ui, st, on_set_palico, |ui, s, f: SharedString, v: i32| {
        let i = view(|v| v.palico_sel);
        if i < 0 {
            return;
        }
        let i = i as usize;
        let t = Target::Palico(i, targets::pal_of(&f));
        if refused(&ui, pal_conf(i)) {
            return;
        }
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, pal_conf(i)), |sv, base| {
            let mut p = palico::get(sv, base, i);
            match f.as_str() {
                "level" => p.level = v.clamp(1, palico::MAX_LEVEL as i32) as u8,
                "exp" => p.exp = v.max(0) as u32,
                "bias" => p.bias = v.clamp(0, 7) as u8,
                "target" => p.target = v.clamp(1, palico::TARGETS.len() as i32 - 1) as u8,
                _ => {}
            }
            palico::set(sv, base, i, &p);
            vec![]
        });
        let _ = &ui;
    });
    // a move or skill of the list ("moves" / "skills"): replace it, or equip / unequip it
    on!(ui, st, on_set_palico_entry, |ui, s, kind: SharedString, slot: i32, id: i32, on: bool| {
        let i = view(|v| v.palico_sel);
        let (Ok(i), Ok(k)) = (usize::try_from(i), usize::try_from(slot)) else { return };
        let moves = kind == "moves";
        let t = Target::Palico(i, if moves { targets::Pal::Moves } else { targets::Pal::Skills });
        if refused(&ui, pal_conf(i)) {
            return;
        }
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, pal_conf(i)), |sv, base| {
            let mut p = palico::get(sv, base, i);
            let id = id.clamp(0, if moves { palico::NO_MOVE } else { palico::NO_SKILL } as i32 - 1) as u8;
            if moves {
                if p.learned.get(k) != Some(&id) {
                    palico::set_list_move(&mut p, k, id);
                } else if id != 0 {
                    palico::equip(&mut p.moves, id, on);
                }
            } else if p.skills.get(k) != Some(&id) {
                palico::set_list_skill(&mut p, k, id);
            } else if id != 0 {
                palico::equip(&mut p.skills_on, id, on);
            }
            palico::set(sv, base, i, &p);
            vec![]
        });
        let _ = &ui;
    });
    // a look: the choice's index, or for a colour its palette entry
    on!(ui, st, on_set_palico_look, |ui, s, key: SharedString, v: i32| {
        let i = view(|v| v.palico_sel);
        let (Ok(i), Ok(v)) = (usize::try_from(i), u8::try_from(v)) else { return };
        let Some(f) = palico::look_fields().into_iter().find(|f| f.key == key.as_str()) else { return };
        let t = Target::Palico(i, targets::Pal::Looks);
        if refused(&ui, pal_conf(i)) {
            return;
        }
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, pal_conf(i)), |sv, base| {
            let mut p = palico::get(sv, base, i);
            f.set(&mut p, if f.colour.is_some() { v } else { v.saturating_add(f.first) });
            palico::set(sv, base, i, &p);
            vec![]
        });
        let _ = &ui;
    });
    on!(ui, st, on_set_palico_text, |ui, s, f: SharedString, t: SharedString| {
        let i = view(|v| v.palico_sel);
        if i < 0 {
            return;
        }
        // a Palico without a name is an empty slot to the game
        if f == "name" && t.trim().is_empty() {
            return toast(&ui, tr("A Palico needs a name"), true);
        }
        let i = i as usize;
        let tg = Target::Palico(i, targets::pal_of(&f));
        if refused(&ui, pal_conf(i)) {
            return;
        }
        let title = tg.label(s.save(), s.slot);
        s.edit(Edit::one(tg, title, pal_conf(i)), |sv, base| {
            let mut p = palico::get(sv, base, i);
            match f.as_str() {
                "name" => p.name = t.to_string(),
                "greeting" => p.greeting = t.to_string(),
                "owner" => p.owner = t.to_string(),
                _ => {}
            }
            palico::set(sv, base, i, &p);
            vec![]
        });
        let _ = &ui;
    });
    // the selected Palico and the equipment it wears, into another character's Palicoes
    on!(ui, st, on_palico_copy, |ui, s, k: i32| {
        let (Ok(i), Ok(k)) = (usize::try_from(view(|v| v.palico_sel)), usize::try_from(k)) else { return };
        let from = s.slot;
        if k > 2 || k == from || !s.save().slot_used(k) || palico::is_empty(s.save(), s.base(), i) {
            return;
        }
        let name = palico::get(s.save(), s.base(), i).name;
        let to = character::get(s.save(), s.save().base(k)).name;
        let file = palico::export(s.save(), s.base(), i);
        add_palico(&ui, &mut s, k, &file, trf("Copy {} to {}", &[&name, &to]));
    });
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_palico_export(move || {
            let Some(ui) = w.upgrade() else { return };
            let (name, bytes) = {
                let s = st.borrow();
                let Ok(i) = usize::try_from(view(|v| v.palico_sel)) else { return };
                if s.doc.is_none() || palico::is_empty(s.save(), s.base(), i) {
                    return;
                }
                (palico::get(s.save(), s.base(), i).name, palico::export(s.save(), s.base(), i))
            };
            save_file(&ui, tr("Export Palico"), (tr("MHGU Palico"), PALICO_EXT), &name, &bytes, (trf("Exported {}", &[&name]), tr("With the equipment it wears")));
        });
    }
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_palico_import(move || {
            let Some(ui) = w.upgrade() else { return };
            if st.borrow().doc.is_none() {
                return;
            }
            let Some(f) = open_file(&ui, tr("Import Palico"), tr("MHGU Palico"), PALICO_EXT) else { return };
            let Ok(name) = palico::name_of(&f) else {
                return toast(&ui, tr("Not imported: the file is not a Palico exported by this editor"), true);
            };
            {
                let mut s = st.borrow_mut();
                let k = s.slot;
                add_palico(&ui, &mut s, k, &f, trf("Import {}", &[&name]));
            }
            refresh(&ui, &st.borrow());
        });
    }
}

/// Stage a Palico file's Palico into character `k`'s Palicoes; says why when it does not
/// fit, and selects it when `k` is the character shown.
fn add_palico(ui: &AppWindow, s: &mut State, k: usize, file: &[u8], title: String) {
    let e = Edit { key: String::new(), title: title.clone(), detail: tr("Palicoes").into(), note: String::new(), conf: Conf::Derived, targets: vec![] };
    if refused(ui, e.conf) {
        return;
    }
    let mut r = Err(palico::ImportError::NotAPalico);
    s.edit_at(k, true, e, |sv, base| {
        r = palico::import(sv, base, file);
        r.iter().map(|&i| Target::Palico(i, targets::Pal::All)).collect()
    });
    match r {
        Ok(i) => {
            if k == s.slot {
                view(|v| v.palico_sel = i as i32);
            }
            toast_full(ui, tr("Added to Review"), &title, "", ToastAct::None, false);
        }
        Err(palico::ImportError::ListFull) => toast(ui, tr("Not added: every Palico place of that character is taken"), true),
        Err(palico::ImportError::BoxFull) => toast(ui, tr("Not added: the Palico equipment box has no room for its equipment"), true),
        Err(palico::ImportError::NotAPalico) => toast(ui, tr("Not imported: the file is not a Palico exported by this editor"), true),
    }
}
