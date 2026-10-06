//! The Quests page: its models and the callbacks that edit it.

use super::*;

pub(super) fn lock_text(l: &Lock) -> (String, bool) {
    match l {
        Lock::Unlocked => (String::new(), false),
        Lock::Rotated => ("listed only in its rotation".into(), false),
        Lock::Event => ("event quest: listed once downloaded".into(), false),
        Lock::Locked(alts) => (format!("locked: needs {}", alts.iter().map(|a| a.join(" and ")).collect::<Vec<_>>().join(", or ")), true),
    }
}

/// Deviant of a Special Permit quest index.
pub(super) fn deviant_of(index: usize) -> Option<usize> {
    (0..DEVIANTS.len()).find(|&d| {
        let (q0, n) = Char::deviant_levels(d);
        (q0..q0 + n).contains(&index)
    })
}

/// Group of a quest on its tab: rank, or the deviant on Special Permit (R9).
pub(super) fn quest_group(q: &mhgu_save::data::Quest) -> String {
    match deviant_of(q.index) {
        Some(d) if q.category == "Special Permit" => format!("dev:{d}"),
        _ => format!("rank:{}", q.rank),
    }
}

pub(super) fn quests_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let c = Char::new(st.save(), st.slot);
    let tabs = quest_tabs();
    let tab = view(|v| v.quest_tab).min(tabs.len().saturating_sub(1));
    let (search, missing) = view(|v| (v.quest_search.to_lowercase(), v.quest_missing));
    let cat = tabs.get(tab).cloned().unwrap_or_default();
    let mut qs: Vec<_> = Char::real_quests(true).into_iter().filter(|q| q.category == cat).collect();
    qs.sort_by_key(|q| (deviant_of(q.index).unwrap_or(0), q.rank.parse::<u32>().unwrap_or(99), q.id));
    let base = st.base();
    let total = qs.len();
    let done = qs.iter().filter(|q| c.quest(QuestBit::Cleared, q.index)).count();
    let mut rows = vec![];
    let mut group = String::from("\0");
    for q in &qs {
        let cl = c.quest(QuestBit::Cleared, q.index);
        if (missing && cl) || (!search.is_empty() && !q.name.to_lowercase().contains(&search) && !q.id.to_string().contains(&search)) {
            continue;
        }
        let g = quest_group(q);
        if g != group {
            group = g.clone();
            let members: Vec<_> = qs.iter().filter(|x| quest_group(x) == g).collect();
            let open = members.iter().any(|x| !c.quest(QuestBit::Cleared, x.index));
            let (label, action) = match deviant_of(q.index) {
                Some(d) if cat == "Special Permit" => (DEVIANTS[d].to_string(), "Mark all cleared".to_string()),
                _ if q.rank.is_empty() => (cat.clone(), "Mark all cleared".to_string()),
                _ => (format!("{cat} {}★", q.rank), format!("Mark {}★ cleared", q.rank)),
            };
            let cleared = members.iter().filter(|x| c.quest(QuestBit::Cleared, x.index)).count();
            rows.push(QuestRow {
                header: true,
                name: format!("{label} · {} / {}", cleared, members.len()).into(),
                group: g.into(),
                action: if open { action } else { String::new() }.into(),
                ..Default::default()
            });
        }
        let (lock, locked) = if cl { (String::new(), false) } else { lock_text(&c.lock(q.id)) };
        let byte = |o: usize| base + o + q.index / 8;
        let changed = [mhgu_save::progress::CLEARED, mhgu_save::progress::SEEN, mhgu_save::progress::FAILED].iter().any(|&o| st.changed(byte(o), 1));
        rows.push(QuestRow {
            index: q.index as i32,
            id: q.id as i32,
            name: q.name.clone().into(),
            sub: format!("#{}{}", q.id, if lock.is_empty() { String::new() } else { format!(" · {lock}") }).into(),
            cleared: cl,
            seen: c.quest(QuestBit::Seen, q.index),
            failed: c.quest(QuestBit::Failed, q.index),
            locked,
            prowler: q.prowler,
            changed,
            was: if changed { st.was(Target::Quest(q.index)) } else { String::new() }.into(),
            header: false,
            group: SharedString::default(),
            action: SharedString::default(),
        });
    }
    api.set_quest_summary(format!("{cat}: {} / {} cleared · confirmed in game except where marked", num(done as i64), num(total as i64)).into());
    api.set_quest_tabs(strings(tabs));
    api.set_quest_tab(tab as i32);
    api.set_quests(model(rows));
}

pub(super) fn wire_quests(ui: &AppWindow, st: &Shared) {
    // quests
    on!(ui, st, on_select_quest_tab, |ui, s, i: i32| {
        view(|v| v.quest_tab = i as usize);
        let _ = (&ui, &s);
    });
    on!(ui, st, on_filter_quests, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.quest_search = api.get_quest_search().to_string();
            v.quest_missing = api.get_quest_missing();
        });
        let _ = &s;
    });
    on!(ui, st, on_set_quest, |ui, s, index: i32, bit: SharedString, on: bool| {
        let which = match bit.as_str() {
            "seen" => QuestBit::Seen,
            "failed" => QuestBit::Failed,
            _ => QuestBit::Cleared,
        };
        let slot = s.slot;
        let t = Target::Quest(index as usize);
        let title = t.label(s.save(), slot);
        let mut sets = vec![];
        let mut e = Edit::one(t, title, Conf::Confirmed);
        e.key = format!("{}:{bit}", t.key());
        s.edit(e, |sv, _| {
            let mut c = Char::new(sv, slot);
            if which == QuestBit::Cleared && on {
                sets = c.clear_quests(&[index as usize]);
            } else {
                c.set_quest(which, index as usize, on);
            }
            vec![]
        });
        if !sets.is_empty() {
            if let Some(o) = s.ops.last_mut() {
                o.note = "Also completes its quest set, as the game does".into();
            }
        }
        let _ = &ui;
    });
    on!(ui, st, on_quest_bulk, |ui, s, group: SharedString| {
        let tabs = quest_tabs();
        let cat = tabs.get(view(|v| v.quest_tab)).cloned().unwrap_or_default();
        let slot = s.slot;
        let members: Vec<usize> = Char::real_quests(true).into_iter().filter(|q| q.category == cat && quest_group(q) == group.as_str()).map(|q| q.index).collect();
        let label = match group.split_once(':') {
            Some(("dev", d)) => format!("{} quests", d.parse::<usize>().ok().and_then(|d| DEVIANTS.get(d)).copied().unwrap_or("Deviant")),
            Some((_, r)) if !r.is_empty() => format!("{cat} {r}★ quests"),
            _ => format!("{cat} quests"),
        };
        let e = Edit { key: String::new(), title: format!("{label} marked cleared"), detail: "Quests".into(), note: "Completed quest sets are recorded too, as the game does".into(), conf: Conf::Confirmed, targets: vec![] };
        s.edit(e, |sv, _| {
            let mut c = Char::new(sv, slot);
            let todo: Vec<usize> = members.iter().copied().filter(|&i| !c.quest(QuestBit::Cleared, i)).collect();
            c.clear_quests(&todo);
            todo.into_iter().map(Target::Quest).collect()
        });
        let _ = &ui;
    });
}
