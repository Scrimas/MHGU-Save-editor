//! The Requests page: its models and the callbacks that edit it.

use super::*;

pub(super) fn requests_page(ui: &AppWindow, st: &State) {
    let api = ui.global::<Api>();
    let c = Char::new(st.save(), st.slot);
    let (f, q) = view(|v| (v.request_filter.clone(), v.request_search.to_lowercase()));
    let base = st.base();
    let (mut open, mut done_n) = (0, 0);
    let rows: Vec<RequestRow> = tables()
        .requests
        .iter()
        .filter_map(|r| {
            let acc = r.accept_flag.is_some_and(|x| c.flag(x));
            let done = r.done_flag.is_some_and(|x| c.flag(x));
            if done { done_n += 1 } else { open += 1 }
            let keep = match f.as_str() {
                "open" => !done,
                "done" => done,
                _ => true,
            };
            let village = match r.village.as_str() {
                "Bherna" | "Kokoto" | "Pokke" | "Yukumo" => r.village.clone(),
                _ => "Hub".to_string(),
            };
            let name = if r.quest_name.is_empty() { "Delivery request".into() } else { r.quest_name.clone() };
            if !keep || (!q.is_empty() && !name.to_lowercase().contains(&q) && !village.to_lowercase().contains(&q)) {
                return None;
            }
            let waiting = if acc { String::new() } else { c.offer_missing(r.index).join("; ") };
            let flag_changed = |x: Option<usize>| x.is_some_and(|x| st.changed(base + mhgu_save::progress::FLAGS + x / 8, 1));
            let changed = flag_changed(r.accept_flag) || flag_changed(r.done_flag);
            Some(RequestRow {
                index: r.index as i32,
                name: name.into(),
                sub: format!("{village} · #{}{}", r.index, if waiting.is_empty() { String::new() } else { format!(" · waits for: {waiting}") }).into(),
                accepted: acc,
                completed: done,
                has_flags: r.accept_flag.is_some(),
                changed,
                was: if changed { st.was(Target::Request(r.index)) } else { String::new() }.into(),
            })
        })
        .collect();
    api.set_request_summary(format!("{} open · {} completed", num(open), num(done_n)).into());
    api.set_requests(model(rows));
}

pub(super) fn wire_requests(ui: &AppWindow, st: &Shared) {
    // requests
    on!(ui, st, on_filter_requests, |ui, s| {
        let api = ui.global::<Api>();
        view(|v| {
            v.request_filter = api.get_request_filter().to_string();
            v.request_search = api.get_request_search().to_string();
        });
        let _ = &s;
    });
    on!(ui, st, on_set_request, |ui, s, index: i32, what: SharedString, on: bool| {
        if refused(&ui, Conf::Derived) {
            return;
        }
        let r = tables().requests.iter().find(|r| r.index == index as usize).unwrap().clone();
        let slot = s.slot;
        let t = Target::Request(r.index);
        let title = t.label(s.save(), slot);
        let mut e = Edit::one(t, title, Conf::Derived);
        e.key = format!("{}:{what}", t.key());
        if what == "completed" && on {
            e.note = "The villager's reward is not handed over in game".into();
        }
        s.edit(e, |sv, _| {
            let mut c = Char::new(sv, slot);
            match (what.as_str(), on) {
                ("accepted", true) => c.accept_request(r.index),
                ("accepted", false) => {
                    if let Some(f) = r.accept_flag {
                        c.set_flag(f, false)
                    }
                }
                (_, v) => {
                    if let Some(f) = r.done_flag {
                        c.set_flag(f, v);
                        if v {
                            c.accept_request(r.index);
                        }
                    }
                }
            }
            vec![]
        });
        let _ = &ui;
    });
}
