//! The Overview page: its models and the callbacks that edit it.

use super::*;

pub(super) fn overview(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let s = st.save();
    let base = st.base();
    let c = Char::new(s, st.slot);
    let t = tables();
    let real = Char::real_quests(true);
    let done = real.iter().filter(|q| c.quest(QuestBit::Cleared, q.index)).count();
    let arts = t.arts.iter().filter(|a| c.art(a.0)).count();
    let dishes = (0..99).filter(|&i| c.dish(i)).count();
    // the same total as Collections: every award of the card (07.1)
    let missing: Vec<String> = t.awards.iter().filter(|a| !c.award(a.0)).map(|a| a.2.clone()).collect();
    let listed: Vec<usize> = (1..=monsters::N).filter(|&i| monsters::meta(i).crown_awards).collect();
    let met = listed.iter().filter(|&&i| {
        let r = monsters::get(s, base, i);
        r.hunts + r.captures > 0
    }).count();
    let gold = listed.iter().filter(|&&i| monsters::crowns(i, monsters::get(s, base, i)).large == 2).count();
    let mini = listed.iter().filter(|&&i| monsters::crowns(i, monsters::get(s, base, i)).mini).count();
    let chs = character::get(s, base);
    let card = |title: &str, n: usize, of: usize, sub: String, page: &str, tab: i32| StatCard {
        title: title.into(),
        value: format!("{} / {}", num(n as i64), num(of as i64)).into(),
        sub: sub.into(),
        progress: if of == 0 { 0.0 } else { n as f32 / of as f32 },
        done: n >= of && of > 0,
        bar: n < of,
        page: page.into(),
        tab,
    };
    let awards_sub = match missing.len() {
        0 => String::new(),
        1 => trf("1 missing: {}", &[&missing[0]]),
        n => trf("{} missing", &[&n]),
    };
    api.set_stats(model(vec![
        card(tr("Quests cleared"), done, real.len(), trf("Village ★{} · Hub ★{}", &[&c.village_star(), &c.hub_star()]), "quests", 0),
        card(tr("Hunter Arts"), arts, t.arts.len(), String::new(), "collections", 0),
        card(tr("Canteen dishes"), dishes, 99, String::new(), "collections", 1),
        card(tr("Awards"), t.awards.len() - missing.len(), t.awards.len(), awards_sub, "collections", 3),
        // what is counted is named, so 79 here and 93 on Monsters can both be right (07.2)
        card(tr("Large monsters met"), met, listed.len(), tr("Guild Card list").into(), "monsters", 0),
        card(tr("Gold crowns"), gold, listed.len(), trn("{} mini crown", "{} mini crowns", mini as i64, &[&num(mini as i64)]), "monsters", 0),
        // values, not progress: no bars (H1.2)
        StatCard {
            title: tr("Hunter Rank").into(),
            value: num(chs.hr).into(),
            sub: if chs.hr >= 999 { trf("Max · {} HR points", &[&num(chs.hr_points)]) } else { trf("{} HR points", &[&num(chs.hr_points)]) }.into(),
            page: "character".into(),
            ..Default::default()
        },
        StatCard {
            title: tr("Zenny").into(),
            value: num(chs.funds).into(),
            sub: if chs.funds >= character::MAX_FUNDS { tr("Max") } else { "" }.into(),
            page: "character".into(),
            ..Default::default()
        },
    ]));
    let mut cards = vec![];
    let mut done_goals = vec![];
    // each plan copies and diffs the whole save: planned again only once it changed
    let key = (st.version, st.slot);
    let plans = match view(|v| v.goal_plans.clone()) {
        Some((k, p)) if k == key => p,
        _ => {
            let p: Vec<GoalView> = goals::GOALS
                .iter()
                .map(|g| {
                    let p = goals::plan(g.id, s, st.slot);
                    GoalView { title: p.title, summary: p.summary, count: p.count, blocked: p.blocked, empty: p.lines.is_empty() }
                })
                .collect();
            view(|v| v.goal_plans = Some((key, p.clone())));
            p
        }
    };
    for (g, p) in goals::GOALS.iter().zip(&plans) {
        let staged = st.ops.iter().find(|o| o.slot == st.slot && o.key == format!("goal:{}", g.id));
        let goal = |state: i32, detail: String, cnt: String, op: i32| Goal {
            id: g.id.into(),
            title: p.title.clone().into(),
            detail: detail.into(),
            count: cnt.into(),
            confidence: conf(g.conf),
            state,
            op,
        };
        match staged {
            Some(o) => cards.push(goal(1, tr("Its changes are in Review. Nothing is written until you press Write.").into(), String::new(), o.id)),
            None if p.blocked => cards.push(goal(3, p.summary.clone(), p.count.clone(), 0)),
            None if p.empty => done_goals.push(goal(2, p.summary.clone(), String::new(), 0)),
            None => cards.push(goal(0, p.summary.clone(), p.count.clone(), 0)),
        }
    }
    api.set_goals(model(cards));
    api.set_goals_done(model(done_goals));
}

pub(super) fn wire_overview(ui: &AppWindow, st: &Shared) {
    let api = ui.global::<Api>();
    // overview: goals and page-level bulk actions share one preview (R3, R4)
    {
        let w = ui.as_weak();
        let st = st.clone();
        api.on_open_preview(move |id| {
            let Some(ui) = w.upgrade() else { return false };
            let s = st.borrow();
            let Some(doc) = s.doc.as_ref() else { return false };
            let id = bulk_id(&ui, &id);
            let p = goals::plan(&id, &doc.save, s.slot);
            let shown = 200;
            let lines: Vec<PreviewLine> = p.lines.iter().take(shown).map(|(l, a, b)| PreviewLine { label: l.into(), old: a.into(), new: b.into() }).collect();
            let empty = p.lines.is_empty();
            ui.global::<Api>().set_preview(Preview {
                id: id.clone().into(),
                title: p.title.clone().into(),
                // a goal's card already says what it does (C6m); a page action says it here
                summary: if empty {
                    trf("Nothing to change. {}", &[&p.summary]).into()
                } else if goals::GOALS.iter().any(|g| g.id == id) {
                    let n = p.lines.len() as i64;
                    trn("{n} value changes. Nothing is written until you press Write.", "{n} values change. Nothing is written until you press Write.", n, &[]).into()
                } else {
                    let n = p.lines.len() as i64;
                    trn("{} {n} value changes. Nothing is written until you press Write.", "{} {n} values change. Nothing is written until you press Write.", n, &[&p.summary]).into()
                },
                confidence: conf(p.conf.unwrap_or(Conf::Confirmed)),
                lines: model(lines),
                more: p.lines.len().saturating_sub(shown) as i32,
                note: p.note.clone().into(),
                tech: if empty { String::new() } else { p.tech.clone() }.into(),
                empty,
                action: p.action.into(),
            });
            true
        });
    }
    on!(ui, st, on_apply_preview, |ui, s, id: SharedString| {
        apply_plan(&ui, &mut s, &id);
    });
}
