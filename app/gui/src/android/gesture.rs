//! Whose touch it is: Slint's (a tap, a list's scroll) or a swipe's.
//!
//! Slint gives a touch as it happens only to the outermost Flickable or
//! SwipeGestureHandler over it; what is inside gets the press 100 ms late or not at all.
//! A list under a swipe handler stops scrolling properly, and the other way round the
//! swipe starts late. So the vendored Android backend shows every touch here first
//! (app/vendor/README.md). Within the first 8 dp this decides:
//! - the finger goes up or down: Slint's from here on, untouched, so lists scroll as ever;
//! - it goes sideways (further than up or down) and something at the press point takes a
//!   swipe that way (ui/mobile/gestures.slint, Claim): the touch ends in Slint without a
//!   click, and the interface follows the finger from the numbers set here (Gest.d, then
//!   its speed when it lets go);
//! - it goes down on a sheet: the same, for dragging the sheet down. A list in the sheet
//!   scrolls to its top first, and the sheet takes over from there in the same touch.
//!
//! Android's back gesture comes through here as well. Its progress is measured from the
//! finger wherever it went down (since Android 15 the callback's own progress stays at 0
//! for a NativeActivity), and the touch ends in Slint without a click, so whatever was
//! under the finger is not tapped.

use crate::{Api, AppWindow, Gest};
use i_slint_backend_android_activity::{TouchAction, TouchSample, TouchVerdict, set_touch_filter};
use slint::{ComponentHandle, Model};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

/// Android's touch slop, in the window's logical pixels (dp).
const SLOP: f32 = 8.0;
/// The kinds of swipe in the order the finger is offered (gestures.slint): the drawer, the
/// snackbar, a list row, the navigation bar, a page's tabs, the destinations.
const SIDEWAYS: [usize; 6] = [0, 1, 2, 3, 4, 5];
/// A sheet, dragged down.
const SHEET: usize = 6;
/// The pages: a sideways drag on them that nothing takes is no tap.
const NO_TAP: usize = 8;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// no finger down
    Idle,
    /// down, within the slop: Slint has it, a tap so far
    Tap,
    /// Slint's for good: a list's scroll, a drag nothing swipes for, a second finger
    Slint,
    /// Slint's, a list on a sheet scrolling toward its top: the sheet takes over there
    Pull,
    /// a swipe the interface follows; `true` up and down
    Swipe(bool),
    /// a sideways drag on a page that nothing takes: ended in Slint, so that it is no tap
    Dead,
    /// Android's back gesture
    Back,
}

struct Touch {
    mode: Mode,
    x0: f32,
    y0: f32,
    x: f32,
    y: f32,
    /// the axis coordinate where the swipe was claimed: the travel counts from there
    base: f32,
    /// where the back gesture found the finger
    back_x: f32,
    /// the latest samples of a swipe: time in ms, axis coordinate
    trail: VecDeque<(i64, f32)>,
    /// end Slint's part of this touch at its next sample (the back gesture has started)
    take: bool,
}

thread_local! {
    static TOUCH: RefCell<Touch> = const {
        RefCell::new(Touch { mode: Mode::Idle, x0: 0.0, y0: 0.0, x: 0.0, y: 0.0, base: 0.0, back_x: 0.0, trail: VecDeque::new(), take: false })
    };
    static UI: RefCell<Option<slint::Weak<AppWindow>>> = const { RefCell::new(None) };
    /// the system's back-gesture zone at the side edges (dp), and the window width it was read at
    static EDGE: Cell<(f32, f32)> = const { Cell::new((20.0, 0.0)) };
}

pub fn wire(ui: &AppWindow) {
    UI.set(Some(ui.as_weak()));
    ui.global::<Gest>().on_haptic(|kind| {
        if let Err(e) = super::haptic(kind) {
            super::log(&format!("haptic: {e}"));
        }
    });
    set_touch_filter(on_touch);
}

fn ui() -> Option<AppWindow> {
    UI.with_borrow(|u| u.as_ref().and_then(|w| w.upgrade()))
}

/// The window's width in logical pixels.
fn width(ui: &AppWindow) -> f32 {
    let w = ui.window();
    w.size().width as f32 / w.scale_factor().max(0.1)
}

/// The width of the back gesture's zone at the side edges, in logical pixels: read again
/// when the window's width has changed (a rotation, a foldable opening).
fn edge(ui: &AppWindow) -> f32 {
    let w = width(ui);
    let (e, at) = EDGE.get();
    if at == w {
        return e;
    }
    // (-1: no insets yet, the window is not attached; asked again next time)
    match super::gesture_edge() {
        Some(px) if px >= 0 => {
            let e = px as f32 / ui.window().scale_factor().max(0.1);
            EDGE.set((e, w));
            e
        }
        _ => e,
    }
}

/// The press is in the text field that has the cursor: a sideways drag there selects text,
/// which is Slint's own, so no swipe starts in it.
fn in_focused_text(ui: &AppWindow, x: f32, y: f32) -> bool {
    let w = i_slint_core::window::WindowInner::from_pub(ui.window());
    let Some(item) = w.focus_item.borrow().upgrade() else { return false };
    if item.downcast::<i_slint_core::items::TextInput>().is_none() {
        return false;
    }
    // (the field's box is a little larger than its text)
    const AROUND: f32 = 12.0;
    let g = item.geometry();
    let o = item.map_to_window(g.origin);
    x >= o.x - AROUND && x < o.x + g.size.width + AROUND && y >= o.y - AROUND && y < o.y + g.size.height + AROUND
}

/// The kind of swipe that offered itself for this touch, that way.
fn offered(g: &Gest, kinds: &[usize], forth: bool) -> Option<usize> {
    let touch = g.get_touch();
    let w = if forth { g.get_wf() } else { g.get_wb() };
    kinds.iter().copied().find(|&k| w.row_data(k) == Some(touch))
}

/// The interface's change handlers, now: the takers offer themselves, start and end in
/// them, and several samples of a touch can come in one turn of the event loop.
fn settle() {
    i_slint_core::properties::ChangeTracker::run_change_handlers();
}

fn begin(t: &mut Touch, g: &Gest, kind: usize, vertical: bool, forth: bool, s: &TouchSample) {
    t.mode = Mode::Swipe(vertical);
    t.base = if vertical { s.y } else { s.x };
    t.trail.clear();
    t.trail.push_back((s.time_ms, t.base));
    g.set_d(0.0);
    g.set_v(0.0);
    g.set_fin(0);
    g.set_forth(forth);
    g.set_claim(kind as i32 + 1);
    settle();
}

/// The finger's speed along the axis over its last 80 ms, in px/s; none after a rest.
fn speed(t: &Touch, now: i64) -> f32 {
    let Some(&(last_t, last_p)) = t.trail.back() else { return 0.0 };
    if now - last_t > 100 {
        return 0.0;
    }
    let from = t.trail.iter().find(|(time, _)| last_t - time <= 80).copied().unwrap_or((last_t, last_p));
    if last_t - from.0 < 8 { 0.0 } else { (last_p - from.1) * 1000.0 / (last_t - from.0) as f32 }
}

fn end(t: &mut Touch, g: &Gest, s: &TouchSample, let_go: bool) {
    if let Mode::Swipe(vertical) = t.mode {
        if let_go {
            g.set_d((if vertical { s.y } else { s.x }) - t.base);
            settle();
        }
        g.set_v(if let_go { speed(t, s.time_ms) } else { 0.0 });
        g.set_fin(if let_go { 1 } else { 2 });
        g.set_claim(0);
        settle();
    }
}

fn on_touch(s: TouchSample) -> TouchVerdict {
    let Some(ui) = ui() else { return TouchVerdict::Pass };
    let g = ui.global::<Gest>();
    TOUCH.with_borrow_mut(|t| {
        let py = t.y;
        t.x = s.x;
        t.y = s.y;
        match s.action {
            TouchAction::Down => {
                // (a swipe whose finger never came up: a touch lost with the window)
                end(t, &g, &s, false);
                *t = Touch {
                    mode: if s.fingers > 1 { Mode::Slint } else { Mode::Tap },
                    x0: s.x,
                    y0: s.y,
                    x: s.x,
                    y: s.y,
                    base: 0.0,
                    back_x: s.x,
                    trail: VecDeque::new(),
                    take: false,
                };
                g.set_sx(s.x);
                g.set_sy(s.y);
                // (a long-press mode whose row went away with its page never ended)
                g.set_holds(0);
                g.set_down(true);
                g.set_touch(g.get_touch().wrapping_add(1).max(0));
                settle();
                TouchVerdict::Pass
            }
            TouchAction::Move => {
                if t.take {
                    // the back gesture has started: Slint's part of the touch ends here
                    t.take = false;
                    let slints = matches!(t.mode, Mode::Tap | Mode::Slint | Mode::Pull);
                    end(t, &g, &s, false);
                    t.mode = Mode::Back;
                    back_progress(&ui, t);
                    return if slints { TouchVerdict::Cancel } else { TouchVerdict::Drop };
                }
                match t.mode {
                    Mode::Tap => {
                        let (dx, dy) = (s.x - t.x0, s.y - t.y0);
                        if s.fingers > 1 {
                            t.mode = Mode::Slint;
                        } else if dx.abs() > SLOP && dx.abs() > dy.abs() {
                            // sideways: a swipe when something there takes one, unless the
                            // press was in the back gesture's zone (that one is Android's)
                            let e = edge(&ui);
                            let free = g.get_holds() == 0 && t.x0 >= e && t.x0 <= width(&ui) - e;
                            // (a row of chips that scrolls sideways, a text field with
                            // the cursor: theirs)
                            let kept = g.get_kept() == g.get_touch() || in_focused_text(&ui, t.x0, t.y0);
                            match (free && !kept).then(|| offered(&g, &SIDEWAYS, dx > 0.0)).flatten() {
                                Some(kind) => {
                                    begin(t, &g, kind, false, dx > 0.0, &s);
                                    return TouchVerdict::Cancel;
                                }
                                // on a page, as past its last tab: nothing moves, and the
                                // row under the finger is not tapped when it lets go
                                None if !kept && g.get_holds() == 0 && offered(&g, &[NO_TAP], dx > 0.0).is_some() => {
                                    t.mode = Mode::Dead;
                                    return TouchVerdict::Cancel;
                                }
                                None => t.mode = Mode::Slint,
                            }
                        } else if dy.abs() > SLOP {
                            if dy > 0.0 && g.get_holds() == 0 {
                                if offered(&g, &[SHEET], true).is_some() {
                                    begin(t, &g, SHEET, true, true, &s);
                                    return TouchVerdict::Cancel;
                                }
                                t.mode = Mode::Pull;
                            } else {
                                t.mode = Mode::Slint;
                            }
                        }
                        TouchVerdict::Pass
                    }
                    Mode::Pull => {
                        // the list has reached its top and the finger still goes down
                        if s.y > py && offered(&g, &[SHEET], true).is_some() {
                            begin(t, &g, SHEET, true, true, &s);
                            return TouchVerdict::Cancel;
                        }
                        TouchVerdict::Pass
                    }
                    Mode::Swipe(vertical) => {
                        let p = if vertical { s.y } else { s.x };
                        t.trail.push_back((s.time_ms, p));
                        while t.trail.len() > 24 {
                            t.trail.pop_front();
                        }
                        g.set_d(p - t.base);
                        TouchVerdict::Drop
                    }
                    Mode::Back => {
                        back_progress(&ui, t);
                        TouchVerdict::Drop
                    }
                    Mode::Dead => TouchVerdict::Drop,
                    Mode::Slint | Mode::Idle => TouchVerdict::Pass,
                }
            }
            TouchAction::Up | TouchAction::Cancel => {
                let let_go = s.action == TouchAction::Up;
                let verdict = match t.mode {
                    Mode::Swipe(_) => {
                        end(t, &g, &s, let_go);
                        TouchVerdict::Drop
                    }
                    // (its progress stays where it is until Android says how the gesture ended)
                    Mode::Back | Mode::Dead => TouchVerdict::Drop,
                    // the back gesture began and no sample came since: still Slint's touch,
                    // which must not end as a tap on what was under the finger
                    _ if t.take => TouchVerdict::Cancel,
                    // a touch Android takes away is not a tap either
                    _ if !let_go => TouchVerdict::Cancel,
                    _ => TouchVerdict::Pass,
                };
                t.mode = Mode::Idle;
                t.take = false;
                g.set_down(false);
                verdict
            }
        }
    })
}

/// How far the back gesture has come: the finger's way from where the gesture found it,
/// over the window's width.
fn back_progress(ui: &AppWindow, t: &Touch) {
    let p = ((t.x - t.back_x).abs() / width(ui).max(1.0)).clamp(0.0, 1.0);
    let api = ui.global::<Api>();
    if p > api.get_back_progress() {
        api.set_back_progress(p);
    }
}

/// Android's back gesture has started (back.rs): the finger is its own from here.
pub fn back_started() {
    let Some(ui) = ui() else { return };
    let g = ui.global::<Gest>();
    TOUCH.with_borrow_mut(|t| {
        if t.mode == Mode::Idle {
            return;
        }
        if matches!(t.mode, Mode::Swipe(_)) {
            // (a swipe that began before Android decided: it springs back)
            let s = TouchSample { action: TouchAction::Cancel, x: t.x, y: t.y, time_ms: 0, fingers: 1 };
            end(t, &g, &s, false);
            t.mode = Mode::Back;
        } else if t.mode != Mode::Back {
            // Slint still has the touch: it ends at the finger's next sample
            t.take = true;
        }
        t.back_x = t.x0;
        back_progress(&ui, t);
    });
}
