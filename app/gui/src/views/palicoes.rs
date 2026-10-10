//! The Palicoes page: its models and the callbacks that edit it.

use super::*;

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
    let all = (0..palico::LIST_N).filter(|&i| !palico::is_empty(s, base, i)).count();
    let rows: Vec<PalicoRow> = (0..palico::LIST_N)
        .filter(|&i| !palico::is_empty(s, base, i))
        .map(|i| (i, palico::get(s, base, i)))
        .filter(|(_, p)| q.is_empty() || p.name.to_lowercase().contains(&q))
        .map(|(i, p)| {
            PalicoRow {
                index: i as i32,
                name: p.name.into(),
                sub: trf("Lv {} · {}", &[&p.level, &palico::BIASES.get(p.bias as usize).map_or("?", |&b| tr(b))]).into(),
                changed: st.changed(base + palico::LIST + palico::RECORD * i, palico::RECORD),
            }
        })
        .collect();
    let mut sel = view(|v| v.palico_sel);
    if sel < 0 {
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
    on!(ui, st, on_set_palico, |ui, s, f: SharedString, v: i32| {
        let i = view(|v| v.palico_sel);
        if i < 0 {
            return;
        }
        let i = i as usize;
        let t = Target::Palico(i, targets::pal_of(&f));
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
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
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
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
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed), |sv, base| {
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
        let title = tg.label(s.save(), s.slot);
        s.edit(Edit::one(tg, title, Conf::Confirmed), |sv, base| {
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
