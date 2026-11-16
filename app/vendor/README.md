# Vendored crates

## i-slint-backend-android-activity 1.18.1

Slint's Android backend, as published on crates.io, with one addition: a touch filter.
`app/Cargo.toml` points Cargo at this copy (`[patch.crates-io]`); it must be the same
version as the `slint` crate (pinned in `app/gui/Cargo.toml`).

### Why

The phone interface follows the finger: tabs and destinations are pagers, sheets are
dragged down, rows and the snackbar are swiped away. In Slint 1.18 a touch reaches only
the outermost `Flickable` or `SwipeGestureHandler` as it happens; whatever is inside gets
the press 100 ms later, or not at all once the finger has moved. A list under a swipe
handler stops scrolling properly, and with the nesting reversed the swipe starts late and
quick flings are lost. Scroll or swipe has to be decided before Slint sees the touch.

Also, Slint turns a touch that Android cancels (its back gesture taking over) into a
release where the finger is, which the `TouchArea` under it reads as a click.

### What differs from upstream (`touch-filter.patch`)

- `lib.rs`: `TouchAction`, `TouchSample`, `TouchVerdict` and `set_touch_filter()`.
- `androidwindowadapter.rs`, `process_inputs`: every Down, Move, Up and Cancel of the
  primary finger is shown to the filter before it is dispatched. `Pass` dispatches it as
  upstream does; `Drop` keeps it from Slint; `Cancel` ends the touch in Slint with a
  release far outside the window (no click) and keeps the rest of that touch from Slint.
  A text field that held the press takes any release for a tap, and with it the cursor
  and the keyboard: around that release the focus is put back where it was and the
  keyboard is not asked (`quiet_ime`, checked in `input_method_request`).

Nothing else is changed; without a filter set, the backend behaves as upstream.
`app/gui/src/android/gesture.rs` is the filter.

### Updating Slint

1. Copy the new version of the crate from `~/.cargo/registry/src/*/` over this folder
   (without `Cargo.lock`, `.cargo-ok`, `.cargo_vcs_info.json`).
2. `patch -p1 -d i-slint-backend-android-activity < touch-filter.patch`, fix what no
   longer applies, and write the patch again from the two files.
3. Check whether upstream now offers its own way to see touches before they are
   dispatched, or lets a `SwipeGestureHandler` and a `Flickable` share a touch without the
   delay: then this copy can go.
