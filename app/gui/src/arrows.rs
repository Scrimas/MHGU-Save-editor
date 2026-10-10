//! Plain arrow keys move the keyboard focus between the entries of the page: to the
//! nearest control in that direction, as on a console menu. A list row selects itself
//! when it gets the focus (ListRow), so Up/Down walk a list and its detail follows.
//!
//! The window offers the arrows here before the focused control sees them (Api.arrow from
//! its capture-key-pressed). They stay with the control while it uses them: a text field
//! moves its cursor with Left/Right, a number box being edited steps with Up/Down too
//! (Escape leaves it, then the arrows go on). The sidebar, the top bar and Review keep
//! their own keys; so does an open popup.
//!
//! The focus stays inside the scroll area it is in (a list, a page): at the last row
//! visible the area scrolls a step first, since a ListView only builds the rows in view;
//! at the end of the area it moves on to the control beyond (the search field above a list).
//! Slint scrolls the control that gets the focus into view.
//!
//! Uses Slint's internal item tree: keep slint and i-slint-core on the same pinned
//! version (Cargo.toml).

use crate::{Api, AppWindow};
use i_slint_core::accessibility::AccessibleStringProperty;
use i_slint_core::input::FocusReason;
use i_slint_core::item_tree::{ItemRc, ParentItemTraversalMode::StopAtPopups};
use i_slint_core::items::{AccessibleRole, Flickable, FocusScope, TextInput};
use i_slint_core::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize};
use i_slint_core::window::WindowInner;
use slint::ComponentHandle;

pub fn install(ui: &AppWindow) {
    let w = ui.as_weak();
    ui.global::<Api>().on_arrow(move |dx, dy, x, y, width, height| {
        let Some(ui) = w.upgrade() else { return false };
        let area = LogicalRect::new(LogicalPoint::new(x, y), LogicalSize::new(width, height));
        step(&ui, (dx.signum(), dy.signum()), area, true)
    });
}

/// Arrow `d` ((-1|0|1, -1|0|1)) inside the page `area` (window coordinates); false leaves
/// the key to the focused control.
fn step(ui: &AppWindow, d: (i32, i32), area: LogicalRect, may_scroll: bool) -> bool {
    let inner = WindowInner::from_pub(ui.window());
    if !inner.active_popups().is_empty() {
        return false;
    }
    let focused = inner.focus_item.borrow().upgrade();
    let origin = match focused {
        // the window's own FocusScope: no entry has the focus yet
        Some(f) if !window_rect(&f).contains_rect(&area) => {
            if !area.contains(window_rect(&f).center()) {
                return false;
            }
            if f.downcast::<TextInput>().is_some() && (d.0 != 0 || in_spinbox(&f)) {
                return false;
            }
            Some(f)
        }
        _ => None,
    };
    let Some(root) = tree_root(ui) else { return false };
    let mut all = Vec::new();
    collect(&root, area, &mut all);
    // nothing focused: from the list's picked row, if the page has one
    let origin = origin.or_else(|| selected_entry(&root).filter(|e| area.contains(window_rect(e).center())));
    let Some(origin) = origin else {
        // the first entry going down or right, the last going up or left
        let key = |r: &LogicalRect| (r.min_y(), r.min_x());
        let pick = if d.0 + d.1 > 0 {
            all.iter().min_by(|a, b| key(&a.1).partial_cmp(&key(&b.1)).unwrap())
        } else {
            all.iter().max_by(|a, b| key(&a.1).partial_cmp(&key(&b.1)).unwrap())
        };
        if let Some((t, _)) = pick {
            land(inner, t);
        }
        return true;
    };
    let o = window_rect(&origin);
    let scroller = flickable(&origin);
    let best = |same: bool| {
        all.iter()
            .filter(|(c, _)| *c != origin && (flickable(c) == scroller) == same)
            .filter_map(|(c, r)| score(&o, r, d, !same).map(|s| (s, c)))
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .map(|(_, c)| c.clone())
    };
    if let Some(t) = best(true) {
        land(inner, &t);
        return true;
    }
    // the next row may not be built yet: scroll a step, then look again
    if may_scroll && let Some(f) = &scroller && scroll(f, d, &o) {
        let w = ui.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(40), move || {
            if let Some(ui) = w.upgrade() {
                step(&ui, d, area, false);
            }
        });
        return true;
    }
    if let Some(t) = best(false) {
        land(inner, &t);
    }
    true
}

/// Entries kept in view beyond the one that gets the focus, in its own heights.
const MARGIN: f32 = 1.5;
/// The glide of the view to its new place.
const GLIDE: std::time::Duration = std::time::Duration::from_millis(160);

/// Gives `t` the focus. Slint scrolls it into view at once, to the very edge; instead the
/// view glides to keep MARGIN entries beyond it in sight, so the list starts moving
/// before the edge and you see what comes next.
fn land(inner: &WindowInner, t: &ItemRc) {
    let Some(f) = flickable(t) else {
        inner.set_focus_item(t, true, FocusReason::TabNavigation);
        return;
    };
    let Some(fl) = f.downcast::<Flickable>() else { return };
    let from = -fl.as_pin_ref().content_y().get();
    let (view, content) = (f.geometry().height(), fl.as_pin_ref().content_height().get());
    let r = window_rect(t);
    let top = r.min_y() - window_rect(&f).min_y() + from;
    let m = (r.height() * MARGIN).min(((view - r.height()) / 2.0).max(0.0));
    let mut to = from;
    if top - m < to {
        to = top - m;
    }
    if top + r.height() + m > to + view {
        to = top + r.height() + m - view;
    }
    let to = to.clamp(0.0, (content - view).max(0.0));
    inner.set_focus_item(t, true, FocusReason::TabNavigation);
    // undo Slint's jump, then glide from where the view was
    set_y(&f, from);
    glide(&f, to);
}

struct Glide {
    flick: i_slint_core::item_tree::ItemWeak,
    from: f32,
    to: f32,
    // Slint's animation clock: in step with the frames (and mocked in tests)
    start: i_slint_core::animations::Instant,
    timer: slint::Timer,
}

thread_local! {
    static GLIDE_NOW: std::cell::RefCell<Option<Glide>> = const { std::cell::RefCell::new(None) };
}

/// Scrolls `f` to `to` (content y, down positive) with an ease-out; a glide still running
/// goes on from where it is.
fn glide(f: &ItemRc, to: f32) {
    let Some(fl) = f.downcast::<Flickable>() else { return };
    let from = -fl.as_pin_ref().content_y().get();
    if (to - from).abs() < 0.5 {
        return;
    }
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(8), || {
        GLIDE_NOW.with(|g| {
            let g = g.borrow();
            let Some(g) = g.as_ref() else { return };
            let t = (i_slint_core::animations::current_tick().duration_since(g.start).as_secs_f32() / GLIDE.as_secs_f32()).min(1.0);
            if let Some(f) = g.flick.upgrade() {
                let ease = 1.0 - (1.0 - t).powi(3);
                set_y(&f, g.from + (g.to - g.from) * ease);
            }
            // stopped, not dropped: a timer may not go away inside its own tick
            if t >= 1.0 || g.flick.upgrade().is_none() {
                g.timer.stop();
            }
        });
    });
    GLIDE_NOW.with(|g| *g.borrow_mut() = Some(Glide { flick: f.downgrade(), from, to, start: i_slint_core::animations::current_tick(), timer }));
}

fn set_y(f: &ItemRc, y: f32) {
    if let Some(fl) = f.downcast::<Flickable>() {
        Flickable::FIELD_OFFSETS.content_y().apply_pin(fl.as_pin_ref()).set(LogicalLength::new(-y));
    }
}

/// How far candidate `c` is from origin `o` going `d`, None when it is not that way. Along
/// the move counts once, a sideways gap twice (a cell straight below beats one below and
/// to the side), and a size unlike the origin's a little (a row goes to the next row, a
/// field in a row to the field below it). Leaving the scroll area needs an overlap.
fn score(o: &LogicalRect, c: &LogicalRect, d: (i32, i32), overlap: bool) -> Option<f32> {
    let (oc, cc) = (o.center(), c.center());
    let (along, side, size) = match d {
        (0, 1) if cc.y > oc.y + 1.0 && c.min_y() > o.min_y() => (c.min_y() - o.max_y(), gap(o.min_x(), o.max_x(), c.min_x(), c.max_x()), (c.width() - o.width()).abs()),
        (0, -1) if cc.y < oc.y - 1.0 && c.max_y() < o.max_y() => (o.min_y() - c.max_y(), gap(o.min_x(), o.max_x(), c.min_x(), c.max_x()), (c.width() - o.width()).abs()),
        (1, 0) if cc.x > oc.x + 1.0 && c.min_x() > o.min_x() => (c.min_x() - o.max_x(), gap(o.min_y(), o.max_y(), c.min_y(), c.max_y()), (c.height() - o.height()).abs()),
        (-1, 0) if cc.x < oc.x - 1.0 && c.max_x() < o.max_x() => (o.min_x() - c.max_x(), gap(o.min_y(), o.max_y(), c.min_y(), c.max_y()), (c.height() - o.height()).abs()),
        _ => return None,
    };
    if overlap && side > 0.0 {
        return None;
    }
    Some(along.max(0.0) + 2.0 * side + 0.02 * size)
}

/// Gap between the spans a0..a1 and b0..b1, 0 when they overlap.
fn gap(a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    (b0 - a1).max(a0 - b1).max(0.0)
}

/// The focusable controls of the page shown, with their place in the window.
fn collect(item: &ItemRc, area: LogicalRect, out: &mut Vec<(ItemRc, LogicalRect)>) {
    if focusable(item) && item.is_visible() {
        let r = window_rect(item);
        // the window's own FocusScope is not an entry
        if r.width() > 0.0 && r.height() > 0.0 && area.contains(r.center()) && !r.contains_rect(&area) {
            out.push((item.clone(), r));
        }
    }
    let mut child = item.first_child();
    while let Some(c) = child {
        collect(&c, area, out);
        child = c.next_sibling();
    }
}

/// What Tab stops at.
fn focusable(item: &ItemRc) -> bool {
    if let Some(f) = item.downcast::<FocusScope>() {
        let f = f.as_pin_ref();
        return f.enabled() && f.focus_on_tab_navigation();
    }
    if let Some(t) = item.downcast::<TextInput>() {
        let t = t.as_pin_ref();
        return t.enabled() && !t.read_only();
    }
    false
}

/// A number box's text (NumberBox is a spinbox): Up/Down step it while it is edited.
fn in_spinbox(item: &ItemRc) -> bool {
    let mut up = item.parent_item(StopAtPopups);
    while let Some(i) = up {
        if i.accessible_role() == AccessibleRole::Spinbox {
            return true;
        }
        up = i.parent_item(StopAtPopups);
    }
    false
}

/// The scroll area around `item`.
fn flickable(item: &ItemRc) -> Option<ItemRc> {
    let mut up = item.parent_item(StopAtPopups);
    while let Some(i) = up {
        if i.downcast::<Flickable>().is_some() {
            return Some(i);
        }
        up = i.parent_item(StopAtPopups);
    }
    None
}

/// Scrolls `f` a step of the origin's size along `d`; false at its end.
fn scroll(f: &ItemRc, d: (i32, i32), o: &LogicalRect) -> bool {
    let Some(fl) = f.downcast::<Flickable>() else { return false };
    let fl = fl.as_pin_ref();
    let g = f.geometry();
    let (pos, view, content, by, field) = if d.1 != 0 {
        (-fl.content_y().get(), g.height(), fl.content_height().get(), o.height() * d.1 as f32, Flickable::FIELD_OFFSETS.content_y())
    } else {
        (-fl.content_x().get(), g.width(), fl.content_width().get(), o.width() * d.0 as f32, Flickable::FIELD_OFFSETS.content_x())
    };
    let to = (pos + by).clamp(0.0, (content - view).max(0.0));
    if (to - pos).abs() < 0.5 {
        return false;
    }
    // a glide would take the view back
    GLIDE_NOW.with(|g| g.borrow_mut().take());
    field.apply_pin(fl).set(LogicalLength::new(-to));
    true
}

/// Before a key press: a focus lost with its control (a list rebuilt after a pick drops
/// its rows) goes back to the page's selected row, else to the window's FocusScope.
/// Without a focus Slint delivers keys to nothing, its shortcuts included.
pub fn refocus(window: &slint::Window) {
    let inner = WindowInner::from_pub(window);
    if !inner.active_popups().is_empty() {
        return;
    }
    let focused = inner.focus_item.borrow().upgrade();
    if focused.is_some_and(|f| f.is_visible()) {
        return;
    }
    let Some(root) = inner.try_component().map(|c| ItemRc::new(c, 0)) else { return };
    let target = selected_entry(&root).or_else(|| find(&root, &|i| i.downcast::<FocusScope>().is_some()));
    if let Some(t) = target {
        // the view stays where it is (a glide may be moving it)
        let f = flickable(&t);
        let y = f.as_ref().and_then(|f| f.downcast::<Flickable>()).map(|fl| -fl.as_pin_ref().content_y().get());
        inner.set_focus_item(&t, true, FocusReason::Programmatic);
        if let (Some(f), Some(y)) = (f, y) {
            set_y(&f, y);
        }
    }
}

/// The keyboard entry of the selected row of a list in view (ListRow).
fn selected_entry(root: &ItemRc) -> Option<ItemRc> {
    let selected = |i: &ItemRc| {
        i.accessible_role() == AccessibleRole::ListItem
            && i.accessible_string_property(AccessibleStringProperty::ItemSelected).as_deref() == Some("true")
            && i.is_visible()
    };
    find(root, &selected).and_then(|row| find(&row, &|i| i.downcast::<FocusScope>().is_some()))
}

/// The first item of the tree under `item` (itself included) that `f` accepts.
fn find(item: &ItemRc, f: &dyn Fn(&ItemRc) -> bool) -> Option<ItemRc> {
    if f(item) {
        return Some(item.clone());
    }
    let mut child = item.first_child();
    while let Some(c) = child {
        if let Some(hit) = find(&c, f) {
            return Some(hit);
        }
        child = c.next_sibling();
    }
    None
}

/// For tests: the scroll position of the selected row's list, the row's place in the
/// window and the list's.
#[cfg(test)]
pub fn probe(ui: &AppWindow) -> Option<(f32, LogicalRect, LogicalRect)> {
    let row = selected_entry(&tree_root(ui)?)?;
    let f = flickable(&row)?;
    let fl = f.downcast::<Flickable>()?;
    Some((-fl.as_pin_ref().content_y().get(), window_rect(&row), window_rect(&f)))
}

fn window_rect(item: &ItemRc) -> LogicalRect {
    let g = item.geometry();
    LogicalRect::new(item.map_to_window(g.origin), g.size)
}

/// The window's item tree.
fn tree_root(ui: &AppWindow) -> Option<ItemRc> {
    let inner = WindowInner::from_pub(ui.window());
    Some(ItemRc::new(inner.try_component()?, 0))
}
