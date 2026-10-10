//! Page navigation: the mouse's Back/Forward buttons walk the pages shown, as in a
//! browser, and Ctrl+Up/Down steps through the sidebar (Api.step-page).
//!
//! The history follows Api.page whoever sets it (sidebar, Review, Overview cards), through
//! `shown`, which the window calls on every page switch.

use crate::targets::PAGES;
use crate::{Api, AppWindow};
use slint::ComponentHandle;
use std::cell::RefCell;

/// Pages kept behind the current one.
const DEPTH: usize = 100;

#[derive(Debug)]
struct History {
    cur: String,
    back: Vec<String>,
    forward: Vec<String>,
}

impl Default for History {
    fn default() -> Self {
        // Api.page's initial value
        Self { cur: "open".into(), back: Vec::new(), forward: Vec::new() }
    }
}

impl History {
    /// The window switched to `page`: a new visit, unless a step of the history put it there.
    fn shown(&mut self, page: &str) {
        if page == self.cur {
            return;
        }
        self.back.push(std::mem::replace(&mut self.cur, page.into()));
        if self.back.len() > DEPTH {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    fn back(&mut self) -> Option<String> {
        let page = self.back.pop()?;
        self.forward.push(std::mem::replace(&mut self.cur, page.clone()));
        Some(page)
    }

    fn forward(&mut self) -> Option<String> {
        let page = self.forward.pop()?;
        self.back.push(std::mem::replace(&mut self.cur, page.clone()));
        Some(page)
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
}

pub fn shown(page: &str) {
    HISTORY.with(|h| h.borrow_mut().shown(page));
}

/// Mouse Back (`forward` false) or Forward button.
pub fn step(forward: bool) {
    let Some(ui) = UI.with(|u| u.borrow().upgrade()) else { return };
    let api = ui.global::<Api>();
    // no save open: only the Open page has anything to show
    if !api.get_loaded() {
        return;
    }
    let page = HISTORY.with(|h| {
        let mut h = h.borrow_mut();
        if forward { h.forward() } else { h.back() }
    });
    if let Some(page) = page {
        api.set_page(page.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_and_forward() {
        let mut h = History::default();
        h.shown("overview");
        h.shown("items");
        h.shown("quests");
        assert_eq!(h.back().as_deref(), Some("items"));
        // the window reports the page the step set: not a new visit
        h.shown("items");
        assert_eq!(h.back().as_deref(), Some("overview"));
        assert_eq!(h.forward().as_deref(), Some("items"));
        assert_eq!(h.forward().as_deref(), Some("quests"));
        assert_eq!(h.forward(), None);
        // a new visit drops the pages ahead
        h.back();
        h.shown("monsters");
        assert_eq!(h.forward(), None);
        assert_eq!(h.back().as_deref(), Some("items"));
        assert_eq!(h.back().as_deref(), Some("overview"));
        assert_eq!(h.back().as_deref(), Some("open"));
        assert_eq!(h.back(), None);
    }

    #[test]
    fn depth_is_capped() {
        let mut h = History::default();
        for i in 0..DEPTH + 10 {
            h.shown(&i.to_string());
        }
        assert_eq!(h.back.len(), DEPTH);
    }
}
