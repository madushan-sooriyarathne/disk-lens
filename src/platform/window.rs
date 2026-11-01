//! Places the main window. GPUI can resize a window but not move it, so this goes
//! through its `NSWindow`.

use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// A frame in AppKit screen coordinates (origin at the bottom left).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A move of a window from `from` to `to`: `apply` makes it, `restore` undoes it.
#[derive(Clone)]
pub struct Moved {
    window: Retained<NSWindow>,
    from: Frame,
    to: Frame,
}

impl Moved {
    pub fn apply(&self, animate: bool) {
        set(&self.window, self.to, animate);
    }

    pub fn restore(self, animate: bool) {
        set(&self.window, self.from, animate);
    }
}

/// Plans a move of `handle`'s window to `place(visible frame of its screen)`. None
/// when the window has no AppKit handle (tests) or no screen.
pub fn move_window(
    handle: &impl HasWindowHandle,
    place: impl FnOnce(Frame) -> Frame,
) -> Option<Moved> {
    let RawWindowHandle::AppKit(h) = handle.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: GPUI hands out the content view of its live window, and this runs on
    // the main thread inside a GPUI callback, so the view is valid for this call.
    let view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
    let window = view.window()?;
    let screen = window.screen()?;
    let from = frame(window.frame());
    let to = place(frame(screen.visibleFrame()));
    Some(Moved { window, from, to })
}

fn frame(r: NSRect) -> Frame {
    Frame {
        x: r.origin.x,
        y: r.origin.y,
        width: r.size.width,
        height: r.size.height,
    }
}

fn set(window: &NSWindow, f: Frame, animate: bool) {
    let rect = NSRect::new(NSPoint::new(f.x, f.y), NSSize::new(f.width, f.height));
    window.setFrame_display_animate(rect, true, animate);
}
