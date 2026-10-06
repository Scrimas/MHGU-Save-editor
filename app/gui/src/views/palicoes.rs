//! The Palicoes page: its models and the callbacks that edit it.

use super::*;

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
                sub: format!("Lv {} · {}", p.level, palico::BIASES.get(p.bias as usize).copied().unwrap_or("?")).into(),
                changed: st.changed(base + palico::LIST + palico::RECORD * i, palico::RECORD),
            }
        })
        .collect();
    let mut sel = view(|v| v.palico_sel);
    if sel < 0 {
        sel = rows.first().map(|r| r.index).unwrap_or(-1);
        view(|v| v.palico_sel = sel);
    }
    api.set_palico_summary(count(all, "Palico", "Palicoes").into());
    api.set_palicoes(model(rows));
    api.set_biases(strings(palico::BIASES.iter().map(|s| s.to_string())));
    api.set_palico_targets(strings(palico::TARGETS[1..].iter().map(|s| s.to_string())));
    let mv = |m: &[u8]| {
        let n = &assets::names().support_moves;
        // 0 is "(No Move)", 57 an empty learned slot
        let v: Vec<String> = m.iter().filter(|&&x| x != 0 && x != palico::NO_MOVE && x != 0xFF).map(|&x| n.get(x as usize).cloned().unwrap_or_else(|| format!("#{x}"))).collect();
        if v.is_empty() { "None".to_string() } else { v.join(", ") }
    };
    api.set_palico(if sel >= 0 {
        let i = sel as usize;
        let p = palico::get(s, base, i);
        let was = |f: targets::Pal| -> SharedString { st.was(Target::Palico(i, f)).into() };
        PalicoDetail {
            index: sel,
            name: p.name.into(),
            level: p.level as i32,
            exp: p.exp as i32,
            bias: p.bias as i32,
            target: p.target as i32,
            greeting: p.greeting.into(),
            owner: p.owner.into(),
            moves: mv(&p.moves).into(),
            learned: mv(&p.learned).into(),
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
        // only Large First of the targets was read off in game
        let c = if f == "target" { Conf::Derived } else { Conf::Confirmed };
        if refused(&ui, c) {
            return;
        }
        s.edit(Edit::one(t, title, c), |sv, base| {
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
    on!(ui, st, on_set_palico_text, |ui, s, f: SharedString, t: SharedString| {
        let i = view(|v| v.palico_sel);
        if i < 0 {
            return;
        }
        // a Palico without a name is an empty slot to the game
        if f == "name" && t.trim().is_empty() {
            return toast(&ui, "A Palico needs a name", true);
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
}
