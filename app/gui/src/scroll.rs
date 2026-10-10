//! Touchpad and mouse-wheel scrolling on Linux.
//!
//! Touchpad: winit hands Wayland/X11 touchpad scrolling to Slint 1:1 in logical pixels,
//! and Slint's scroll views drop the delta of the first event of each gesture. Most
//! other toolkits amplify the compositor's touchpad delta several times (GTK3 counts it
//! in wheel steps of page^(2/3) px, Chromium scales it too), so next to them a 1:1 view
//! feels about 7x slower. The hook scales the pixel deltas like they do and re-sends them
//! with their gesture phase, so Slint's momentum after lifting the fingers still works.
//! The compositor's own factor (e.g. Hyprland's touchpad:scroll_factor) is already in
//! these deltas, as for every other app. MHGU_SCROLL_SPEED overrides the factor.
//!
//! Mouse wheel: winit reports a wheel as whole notches (Wayland's discrete steps) and
//! drops the continuous value, which is the one the compositor scales. Slint makes a
//! notch 60 px, so the compositor's wheel factor is lost. On Hyprland the hook reads
//! input:scroll_factor once and applies it; elsewhere notches stay 60 px.
//! MHGU_WHEEL_SPEED overrides the factor.
//!
//! Uses Slint's internal event type: keep slint and i-slint-core on the same pinned
//! version (Cargo.toml).

use i_slint_core::input::{BackendMouseEvent, TouchPhase};
use i_slint_core::lengths::LogicalPoint;
use slint::platform::WindowEvent;
use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
use std::cell::Cell;
use std::rc::Rc;

/// Matched by hand against GTK3 and Chromium apps on Hyprland (touchpad:scroll_factor 0.15).
pub const DEFAULT_SPEED: f32 = 7.0;
/// Slint's own pixels per wheel notch.
const NOTCH_PX: f32 = 60.0;

fn env_factor(name: &str) -> Option<f32> {
    std::env::var(name).ok().and_then(|v| v.parse::<f32>().ok()).filter(|v| *v > 0.0)
}

/// Hyprland's input:scroll_factor, when running under Hyprland.
fn compositor_wheel_factor() -> Option<f32> {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let out = std::process::Command::new("hyprctl").args(["-j", "getoption", "input:scroll_factor"]).output().ok()?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v.get("float").and_then(|f| f.as_f64()).map(|f| f as f32).filter(|f| *f > 0.0)
}

pub fn install(window: &slint::Window) {
    let speed = env_factor("MHGU_SCROLL_SPEED").unwrap_or(DEFAULT_SPEED);
    let wheel = env_factor("MHGU_WHEEL_SPEED").or_else(compositor_wheel_factor).unwrap_or(1.0) * NOTCH_PX;
    let cursor = Rc::new(Cell::new(LogicalPoint::default()));
    window.on_winit_window_event(move |w, event| {
        let position = cursor.get();
        let send = |delta_x: f32, delta_y: f32, phase: TouchPhase| {
            w.dispatch_event(WindowEvent::internal(BackendMouseEvent::Wheel { position, delta_x, delta_y, phase }));
        };
        match event {
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(w.scale_factor() as f64);
                cursor.set(LogicalPoint::new(p.x, p.y));
                EventResult::Propagate
            }
            winit::event::WindowEvent::DroppedFile(p) => {
                crate::nav::dropped(p);
                EventResult::PreventDefault
            }
            // Slint takes a single winit hook: the click-outside focus rule, the mouse's
            // Back/Forward buttons and dropped files ride on this one
            winit::event::WindowEvent::MouseInput { button: b @ (winit::event::MouseButton::Back | winit::event::MouseButton::Forward), state, .. } => {
                if *state == winit::event::ElementState::Pressed {
                    crate::nav::step(*b == winit::event::MouseButton::Forward);
                }
                EventResult::PreventDefault
            }
            winit::event::WindowEvent::KeyboardInput { event: winit::event::KeyEvent { state: winit::event::ElementState::Pressed, .. }, .. } => {
                crate::arrows::refocus(w);
                EventResult::Propagate
            }
            winit::event::WindowEvent::MouseInput { state: winit::event::ElementState::Pressed, .. } => {
                crate::focus::press(w, position);
                EventResult::Propagate
            }
            winit::event::WindowEvent::MouseWheel { delta: winit::event::MouseScrollDelta::PixelDelta(d), phase, .. } => {
                let d = d.to_logical::<f32>(w.scale_factor() as f64);
                match phase {
                    winit::event::TouchPhase::Started => {
                        // open the gesture, then apply its first delta instead of losing it
                        send(0.0, 0.0, TouchPhase::Started);
                        send(d.x * speed, d.y * speed, TouchPhase::Moved);
                    }
                    winit::event::TouchPhase::Moved => send(d.x * speed, d.y * speed, TouchPhase::Moved),
                    winit::event::TouchPhase::Ended => send(d.x * speed, d.y * speed, TouchPhase::Ended),
                    winit::event::TouchPhase::Cancelled => send(d.x * speed, d.y * speed, TouchPhase::Cancelled),
                }
                EventResult::PreventDefault
            }
            winit::event::WindowEvent::MouseWheel { delta: winit::event::MouseScrollDelta::LineDelta(x, y), .. } => {
                // no phase: Slint animates each notch as a wheel step
                send(x * wheel, y * wheel, TouchPhase::Moved);
                EventResult::PreventDefault
            }
            _ => EventResult::Propagate,
        }
    });
}
