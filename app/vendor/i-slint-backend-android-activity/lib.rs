// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidwindowadapter javahelper
#![doc = include_str!("README.md")]
#![doc(html_logo_url = "https://slint.dev/logo/slint-logo-square-light.svg")]
#![cfg_attr(not(target_os = "android"), allow(rustdoc::broken_intra_doc_links))]
#![cfg(target_os = "android")]
#![cfg_attr(slint_nightly_test, feature(non_exhaustive_omitted_patterns_lint))]
#![cfg_attr(slint_nightly_test, warn(non_exhaustive_omitted_patterns))]

mod androidwindowadapter;
mod javahelper;
mod vsync;

#[cfg(all(not(feature = "aa-06"), feature = "aa-05"))]
pub use android_activity_05 as android_activity;
#[cfg(feature = "aa-06")]
pub use android_activity_06 as android_activity;

#[cfg(all(not(feature = "aa-06"), feature = "aa-05"))]
use ndk_08 as ndk;
#[cfg(feature = "aa-06")]
use ndk_09 as ndk;

pub use android_activity::AndroidApp;
use android_activity::PollEvent;
use androidwindowadapter::AndroidWindowAdapter;
use core::ops::ControlFlow;
use core::time::Duration;
use i_slint_core::api::{EventLoopError, PlatformError};
use i_slint_core::platform::{Clipboard, WindowAdapter};
use i_slint_renderer_skia::SkiaRendererExt;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};

thread_local! {
    static CURRENT_WINDOW: RefCell<Weak<AndroidWindowAdapter>> = RefCell::new(Default::default());
}

pub struct AndroidPlatform {
    app: AndroidApp,
    window: Rc<AndroidWindowAdapter>,
    event_listener: Option<Box<dyn Fn(&PollEvent<'_>)>>,
    context: core::cell::OnceCell<i_slint_core::SlintContextWeak>,
}

impl AndroidPlatform {
    /// Instantiate a new Android backend given the [`android_activity::AndroidApp`]
    ///
    /// Pass the returned value to [`slint::platform::set_platform()`](`i_slint_core::platform::set_platform()`)
    ///
    /// # Example
    /// ```
    /// #[cfg(target_os = "android")]
    /// #[unsafe(no_mangle)]
    /// fn android_main(app: i_slint_backend_android_activity::AndroidApp) {
    ///     slint::platform::set_platform(Box::new(
    ///         i_slint_backend_android_activity::AndroidPlatform::new(app),
    ///     ))
    ///     .unwrap();
    ///     // ... your slint application ...
    /// }
    /// ```
    pub fn new(app: AndroidApp) -> Self {
        let window = AndroidWindowAdapter::new(app.clone());
        CURRENT_WINDOW.set(Rc::downgrade(&window));
        Self { app, window, event_listener: None, context: Default::default() }
    }

    /// Instantiate a new Android backend given the [`android_activity::AndroidApp`]
    /// and a function to process the events.
    ///
    /// This is the same as [`AndroidPlatform::new()`], but it allow you to get notified
    /// of events.
    ///
    /// Pass the returned value to [`slint::platform::set_platform()`](`i_slint_core::platform::set_platform()`)
    ///
    /// # Example
    /// ```
    /// #[cfg(target_os = "android")]
    /// #[unsafe(no_mangle)]
    /// fn android_main(app: i_slint_backend_android_activity::AndroidApp) {
    ///     slint::platform::set_platform(Box::new(
    ///         i_slint_backend_android_activity::AndroidPlatform::new_with_event_listener(
    ///             app,
    ///             |event| { eprintln!("got event {event:?}") }
    ///         ),
    ///     ))
    ///     .unwrap();
    ///     // ... your slint application ...
    /// }
    /// ```
    pub fn new_with_event_listener(
        app: AndroidApp,
        listener: impl Fn(&PollEvent<'_>) + 'static,
    ) -> Self {
        let mut this = Self::new(app);
        this.event_listener = Some(Box::new(listener));
        this
    }
}

impl i_slint_core::platform::Platform for AndroidPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }
    fn run_event_loop(&self) -> Result<(), PlatformError> {
        let ctx = self
            .context
            .get()
            .and_then(|ctx| ctx.upgrade())
            .expect("the event loop runs inside the context that owns this platform");
        let vsync = vsync::VsyncDriver::new(self.app.create_waker());
        loop {
            let mut timeout = ctx.duration_until_next_timer_update();
            // While animating, the vsync thread wakes this loop at each display refresh
            // so rendering tracks the refresh rate instead of a fixed poll timeout.
            let animating = self.window.window.has_active_animations();
            vsync.set_animating(animating);
            if animating && !vsync.is_driving() {
                // The vsync thread is not driving frames (still starting, or unavailable);
                // fall back to a periodic wakeup so animations still advance.
                let frame_duration = Duration::from_millis(10);
                timeout = Some(timeout.map_or(frame_duration, |x| x.min(frame_duration)));
            }
            // `request_redraw()` only raises the flag, so a request issued outside the
            // poll would otherwise wait here for an unrelated wakeup.
            if self.window.pending_redraw.get() {
                timeout = Some(Duration::ZERO);
            }
            let mut r = Ok(ControlFlow::Continue(()));
            self.app.poll_events(timeout, |e| {
                ctx.update_timers_and_animations();
                r = self.window.process_event(&e);
                if let Some(event_listener) = &self.event_listener {
                    event_listener(&e)
                }
            });
            if r?.is_break() {
                break;
            }
            if self.window.pending_redraw.take() {
                self.window.do_render()?;
            }
        }
        Ok(())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn i_slint_core::platform::EventLoopProxy>> {
        Some(Box::new(AndroidEventLoopProxy {
            event_queue: self.window.event_queue.clone(),
            waker: self.app.create_waker(),
        }))
    }

    fn set_clipboard_text(&self, text: &str, clipboard: Clipboard) {
        if clipboard == Clipboard::DefaultClipboard {
            self.window
                .java_helper
                .set_clipboard(text)
                .unwrap_or_else(|e| javahelper::print_jni_error(&self.app, e));
        }
    }

    fn clipboard_text(&self, clipboard: Clipboard) -> Option<String> {
        if clipboard == Clipboard::DefaultClipboard {
            Some(
                self.window
                    .java_helper
                    .get_clipboard()
                    .unwrap_or_else(|e| javahelper::print_jni_error(&self.app, e)),
            )
        } else {
            None
        }
    }

    fn bind_context(&self, ctx: i_slint_core::SlintContextWeak, _: i_slint_core::InternalToken) {
        let _ = self.context.set(ctx.clone());
        let ctx = ctx.upgrade().expect("bind_context called while the SlintContext is still alive");
        let color_scheme = match self
            .window
            .java_helper
            .color_scheme()
            .unwrap_or_else(|e| javahelper::print_jni_error(&self.app, e))
        {
            0x10 => i_slint_core::items::ColorScheme::Light, // UI_MODE_NIGHT_NO
            0x20 => i_slint_core::items::ColorScheme::Dark,  // UI_MODE_NIGHT_YES
            _ => i_slint_core::items::ColorScheme::Unknown,
        };
        ctx.set_color_scheme(color_scheme);
        if let Ok(accent) = self.window.java_helper.accent_color() {
            ctx.set_accent_color(accent);
        }
        if let Ok(scale) = self.window.java_helper.font_scale()
            && let Some(size) = javahelper::font_scale_to_logical_length(scale)
        {
            ctx.set_platform_default_font_size(Some(size));
        }
    }

    fn long_press_interval(&self, _: i_slint_core::InternalToken) -> Duration {
        self.window.java_helper.long_press_timeout().unwrap_or(Duration::from_millis(500))
    }
}

enum Event {
    Quit,
    Other(Box<dyn FnOnce() + Send + 'static>),
}

type EventQueue = Arc<Mutex<Vec<Event>>>;

struct AndroidEventLoopProxy {
    event_queue: EventQueue,
    waker: android_activity::AndroidAppWaker,
}

impl i_slint_core::platform::EventLoopProxy for AndroidEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), EventLoopError> {
        self.event_queue.lock().unwrap().push(Event::Quit);
        self.waker.wake();
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), EventLoopError> {
        self.event_queue.lock().unwrap().push(Event::Other(event));
        self.waker.wake();
        Ok(())
    }
}

// ---- Not in upstream Slint (vendor/README.md): the application's touch filter ----

/// What the primary finger did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchAction {
    Down,
    Move,
    Up,
    /// Android took the touch away (its back gesture, a system dialog)
    Cancel,
}

/// One sample of the primary finger, in the window's logical pixels.
#[derive(Clone, Copy, Debug)]
pub struct TouchSample {
    pub action: TouchAction,
    pub x: f32,
    pub y: f32,
    /// the event's time, in milliseconds
    pub time_ms: i64,
    /// fingers on the screen, this one included
    pub fingers: usize,
}

/// What Slint does with a sample the application's filter has seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchVerdict {
    /// dispatch it as usual
    Pass,
    /// keep it from Slint (the application follows the finger itself)
    Drop,
    /// end the touch in Slint here, without the release a cancelled touch is otherwise
    /// turned into (which a TouchArea under the finger reads as a click), and keep the
    /// rest of this touch from Slint
    Cancel,
}

thread_local! {
    static TOUCH_FILTER: RefCell<Option<Box<dyn FnMut(TouchSample) -> TouchVerdict>>> = const { RefCell::new(None) };
    /// the filter took the current touch: its other fingers stay out of Slint too
    pub(crate) static TOUCH_TAKEN: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Shows every touch of the primary finger to `filter` before Slint dispatches it. Called
/// on the event loop's thread, from where the UI can be read and changed.
pub fn set_touch_filter(filter: impl FnMut(TouchSample) -> TouchVerdict + 'static) {
    TOUCH_FILTER.with(|f| *f.borrow_mut() = Some(Box::new(filter)));
}

pub(crate) fn filter_touch(sample: TouchSample) -> TouchVerdict {
    // (taken out while it runs: it may end up back in the event handling)
    let Some(mut filter) = TOUCH_FILTER.with(|f| f.borrow_mut().take()) else { return TouchVerdict::Pass };
    let verdict = filter(sample);
    TOUCH_FILTER.with(|f| {
        let mut slot = f.borrow_mut();
        if slot.is_none() {
            *slot = Some(filter);
        }
    });
    verdict
}

pub fn set_requested_graphics_api(
    requested_graphics_api: Option<i_slint_core::graphics::RequestedGraphicsAPI>,
) -> Result<(), PlatformError> {
    let Some(adapter) = CURRENT_WINDOW.with_borrow(|x| x.upgrade()) else {
        return Err(format!("On Android a graphics API for Slint can only be requested after calling slint::android::init()").into());
    };
    adapter.set_requested_graphics_api(requested_graphics_api);
    Ok(())
}
