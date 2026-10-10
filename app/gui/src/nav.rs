//! Page navigation: the mouse's Back/Forward buttons walk the pages and tabs shown, as in
//! a browser, and Ctrl+Up/Down steps through the sidebar (Api.step-page).
//!
//! The history follows Api.page and the page's tab whoever sets them (sidebar, TabBar,
//! Review, Overview cards), through `shown`, which the window calls on every page switch
//! and a TabBar on every tab switch (Api.tab-shown).

use crate::targets::PAGES;
use crate::{Api, AppWindow, Tabs};
use slint::ComponentHandle;
use std::cell::RefCell;

/// Places kept behind the current one.
const DEPTH: usize = 100;

/// A page and its tab (-1 for a page without tabs).
type Spot = (String, i32);

#[derive(Debug)]
struct History {
    cur: Spot,
    back: Vec<Spot>,
    forward: Vec<Spot>,
}

impl Default for History {
    fn default() -> Self {
        // Api.page's initial value
        Self { cur: ("open".into(), -1), back: Vec::new(), forward: Vec::new() }
    }
}

impl History {
    /// The window shows `spot`: a new visit, unless a step of the history put it there.
    fn shown(&mut self, spot: Spot) {
        if spot == self.cur {
            return;
        }
        self.back.push(std::mem::replace(&mut self.cur, spot));
        if self.back.len() > DEPTH {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    fn back(&mut self) -> Option<Spot> {
        let spot = self.back.pop()?;
        self.forward.push(std::mem::replace(&mut self.cur, spot.clone()));
        Some(spot)
    }

    fn forward(&mut self) -> Option<Spot> {
        let spot = self.forward.pop()?;
        self.back.push(std::mem::replace(&mut self.cur, spot.clone()));
        Some(spot)
    }
}

thread_local! {
    static HISTORY: RefCell<History> = RefCell::default();
    static UI: RefCell<slint::Weak<AppWindow>> = RefCell::default();
}

pub fn install(ui: &AppWindow) {
    UI.with(|u| *u.borrow_mut() = ui.as_weak());
    let w = ui.as_weak();
    ui.global::<Api>().on_step_page(move |d| {
        let Some(ui) = w.upgrade() else { return };
        let api = ui.global::<Api>();
        let page = api.get_page();
        // from the Open page, Down goes to the first page
        let to = match PAGES.iter().position(|p| p.0 == page.as_str()) {
            Some(i) => (i as i32 + d).clamp(0, PAGES.len() as i32 - 1) as usize,
            None if d > 0 => 0,
            None => return,
        };
        api.set_page(PAGES[to].0.into());
    });
    let w = ui.as_weak();
    ui.global::<Api>().on_tab_shown(move || {
        if let Some(ui) = w.upgrade() {
            shown(&ui);
        }
    });
}

/// The page or its tab changed.
pub fn shown(ui: &AppWindow) {
    let page = ui.global::<Api>().get_page();
    let tab = ui.global::<Tabs>().invoke_current(page.clone());
    HISTORY.with(|h| h.borrow_mut().shown((page.into(), tab)));
}

/// Mouse Back (`forward` false) or Forward button.
pub fn step(forward: bool) {
    let Some(ui) = UI.with(|u| u.borrow().upgrade()) else { return };
    let api = ui.global::<Api>();
    // no save open: only the Open page has anything to show
    if !api.get_loaded() {
        return;
    }
    let spot = HISTORY.with(|h| {
        let mut h = h.borrow_mut();
        if forward { h.forward() } else { h.back() }
    });
    let Some((page, tab)) = spot else { return };
    api.set_page(page.as_str().into());
    // the tab as a click on it picks it (filters and all)
    let tabs = ui.global::<Tabs>();
    if tab >= 0 && tabs.invoke_current(page.as_str().into()) != tab {
        tabs.invoke_select(page.into(), tab);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(page: &str, tab: i32) -> Spot {
        (page.into(), tab)
    }

    #[test]
    fn back_and_forward() {
        let mut h = History::default();
        h.shown(at("overview", -1));
        h.shown(at("items", 0));
        h.shown(at("items", 2));
        h.shown(at("quests", 0));
        assert_eq!(h.back(), Some(at("items", 2)));
        // the window reports the place the step set: not a new visit
        h.shown(at("items", 2));
        assert_eq!(h.back(), Some(at("items", 0)));
        assert_eq!(h.back(), Some(at("overview", -1)));
        assert_eq!(h.forward(), Some(at("items", 0)));
        assert_eq!(h.forward(), Some(at("items", 2)));
        assert_eq!(h.forward(), Some(at("quests", 0)));
        assert_eq!(h.forward(), None);
        // a new visit drops the places ahead
        h.back();
        h.shown(at("items", 1));
        assert_eq!(h.forward(), None);
        assert_eq!(h.back(), Some(at("items", 2)));
        assert_eq!(h.back(), Some(at("items", 0)));
        assert_eq!(h.back(), Some(at("overview", -1)));
        assert_eq!(h.back(), Some(at("open", -1)));
        assert_eq!(h.back(), None);
    }

    #[test]
    fn depth_is_capped() {
        let mut h = History::default();
        for i in 0..DEPTH + 10 {
            h.shown(at("database", i as i32));
        }
        assert_eq!(h.back.len(), DEPTH);
    }
}
