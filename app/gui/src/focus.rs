//! Clicking outside a field takes the keyboard focus off it, as in other toolkits.
//!
//! Slint keeps the focus until another focusable item is clicked, so a field stayed
//! selected (and a ComboBox kept its focus border) after clicking elsewhere. On each mouse
//! press outside the focused control, the focus moves to the nearest FocusScope above it:
//! the window's shortcuts (Ctrl+S...) or the open dialog's Escape keep working, and a
//! NumberBox stages its typed value as when Tab leaves it.
//!
//! Uses Slint's internal item tree: keep slint and i-slint-core on the same pinned
//! version (Cargo.toml).

use i_slint_core::input::FocusReason;
use i_slint_core::item_tree::{ItemRc, ParentItemTraversalMode::StopAtPopups};
use i_slint_core::items::FocusScope;
use i_slint_core::lengths::{LogicalPoint, LogicalRect};
use i_slint_core::window::WindowInner;

/// Off Linux; there scroll.rs owns the winit hook (Slint takes one) and calls `press`.
#[cfg(not(target_os = "linux"))]
pub fn install(window: &slint::Window) {
    use slint::winit_030::winit::event::{ElementState, WindowEvent};
    use slint::winit_030::{EventResult, WinitWindowAccessor};
    let cursor = std::cell::Cell::new(LogicalPoint::default());
    window.on_winit_window_event(move |w, event| {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(w.scale_factor() as f64);
                cursor.set(LogicalPoint::new(p.x, p.y));
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, .. } => press(w, cursor.get()),
            _ => {}
        }
        EventResult::Propagate
    });
}

/// A mouse press at `position` (window coordinates), before Slint handles it.
pub fn press(window: &slint::Window, position: LogicalPoint) {
    let inner = WindowInner::from_pub(window);
    // an open ComboBox list or menu handles its own clicks
    if !inner.active_popups().is_empty() {
        return;
    }
    let Some(focused) = inner.focus_item.borrow().upgrade() else { return };
    // the control around the focused item: a NumberBox with its arrows, a LineEdit's frame
    let control = focused.parent_item(StopAtPopups).unwrap_or_else(|| focused.clone());
    if window_rect(&control).contains(position) {
        return;
    }
    let mut up = focused.parent_item(StopAtPopups);
    while let Some(item) = up {
        if item.downcast::<FocusScope>().is_some() {
            inner.set_focus_item(&item, true, FocusReason::PointerClick);
            return;
        }
        up = item.parent_item(StopAtPopups);
    }
    inner.set_focus_item(&focused, false, FocusReason::PointerClick);
}

fn window_rect(item: &ItemRc) -> LogicalRect {
    let g = item.geometry();
    LogicalRect::new(item.map_to_window(g.origin), g.size)
}
