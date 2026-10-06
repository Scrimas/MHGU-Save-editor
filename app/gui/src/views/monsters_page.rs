//! The Monsters page: its models and the callbacks that edit it.

use super::*;

/// Large monster that can still get a crown.
pub(super) fn misses_crown(i: usize, r: monsters::Record) -> bool {
    let c = monsters::crowns(i, r);
    monsters::meta(i).size_record && monsters::meta(i).family_of.is_none() && (!c.mini && !monsters::meta(i).fixed_size || c.large < 2)
}

pub(super) fn monsters_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let (f, large, missing) = view(|v| (v.monster_filter.to_lowercase(), v.monster_large, v.monster_missing));
    let t = tables();
    let mut n_large = 0;
    let mut listed = 0;
    let was = |i: usize, m: Mon| -> SharedString { st.was(Target::Monster(i, m)).into() };
    let rows: Vec<MonsterRow> = t
        .monsters
        .iter()
        .filter(|m| !m.name.is_empty() && !m.name.starts_with("dummy"))
        .filter(|m| !(106..=112).contains(&m.index))
        .filter_map(|m| {
            let meta = monsters::meta(m.index);
            let r = monsters::get(s, base, m.index);
            n_large += m.large as usize;
            listed += meta.crown_awards as usize;
            if (large && !m.large) || (missing && !misses_crown(m.index, r)) {
                return None;
            }
            if !f.is_empty() && !m.name.to_lowercase().contains(&f) {
                return None;
            }
            let (img, has) = icon(assets::monster_icon(m.index));
            let crown = monsters::crowns(m.index, r);
            let mut sub = vec![format!("#{}", m.index)];
            if !meta.class.is_empty() {
                sub.push(meta.class.clone());
            } else if !m.large {
                sub.push("small monster".into());
            }
            if let Some(h) = meta.family_of {
                sub.push(format!("size kept by {}", t.monsters[h - 1].name));
            } else if let Some(b) = meta.base_cm.filter(|_| meta.size_record && r.max > 0 && r.min > 0) {
                sub.push(format!("{:.0}–{:.0} cm", b * r.min as f32 / 100.0, b * r.max as f32 / 100.0));
            }
            let thresholds = if meta.fixed_size {
                "fixed size: any record is gold".to_string()
            } else {
                format!("mini ≤ {} % · silver ≥ {} % · gold ≥ {} %", meta.mini_le, meta.silver_ge, meta.gold_ge)
            };
            let notes = monsters::notes(s, base, m.index);
            let changed = st.changed(base + monsters::HUNTS + 2 * m.index, 2)
                || st.changed(base + monsters::CAPTURES + 2 * m.index, 2)
                || st.changed(base + monsters::SIZES + 4 * m.index, 4)
                || meta.notes_bit.is_some_and(|b| st.changed(base + monsters::NOTES + b / 8, 1));
            let w = |f: Mon| if changed { was(m.index, f) } else { SharedString::default() };
            Some(MonsterRow {
                index: m.index as i32,
                name: m.name.clone().into(),
                icon: img,
                has_icon: has,
                large: m.large,
                has_size: meta.size_record,
                sub: sub.join(" · ").into(),
                hunts: r.hunts as i32,
                captures: r.captures as i32,
                min: r.min as i32,
                max: r.max as i32,
                mini: crown.mini,
                crown: crown.large as i32,
                notes: notes.unwrap_or(false),
                has_notes: notes.is_some(),
                thresholds: thresholds.into(),
                was_hunts: w(Mon::Hunts),
                was_captures: w(Mon::Captures),
                was_min: w(Mon::Min),
                was_max: w(Mon::Max),
                was_notes: w(Mon::Notes),
                changed,
            })
        })
        .collect();
    api.set_monster_summary(
        format!("{} large monsters, {} of them on the Guild Card list · {} shown · confirmed in game except where marked", n_large, listed, rows.len()).into(),
    );
    api.set_monsters(model(rows));
}

pub(super) fn wire_monsters(ui: &AppWindow, st: &Shared) {
    // monsters
    on!(ui, st, on_filter_monsters, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.monster_filter = api.get_monster_filter().to_string();
            v.monster_large = api.get_monster_large_only();
            v.monster_missing = api.get_monster_missing();
        });
        let _ = &s;
    });
    on!(ui, st, on_set_monster, |ui, s, i: i32, field: SharedString, v: i32| {
        let i = i as usize;
        let f = field.as_str();
        let m = match f {
            "hunts" => Mon::Hunts,
            "captures" => Mon::Captures,
            "min" => Mon::Min,
            "max" => Mon::Max,
            _ => Mon::Notes,
        };
        let t = Target::Monster(i, m);
        let title = t.label(s.save(), s.slot);
        s.edit(Edit::one(t, title, Conf::Confirmed).note("Also rebuilds the Guild Card monster log"), |sv, base| {
            if m == Mon::Notes {
                monsters::set_notes(sv, base, i, v != 0);
                return vec![];
            }
            let mut r = monsters::get(sv, base, i);
            let v16 = v.clamp(0, 9999) as u16;
            let mut more = vec![];
            match m {
                Mon::Hunts => r.hunts = v16,
                Mon::Captures => r.captures = v16,
                Mon::Min => {
                    r.min = v16;
                    if r.max < v16 {
                        r.max = v16;
                        more.push(Target::Monster(i, Mon::Max));
                    }
                }
                _ => {
                    r.max = v16;
                    if r.min == 0 || r.min > v16 {
                        r.min = v16.min(r.min.max(1));
                        more.push(Target::Monster(i, Mon::Min));
                    }
                }
            }
            monsters::set(sv, base, i, r);
            more
        });
        let _ = &ui;
    });
}
