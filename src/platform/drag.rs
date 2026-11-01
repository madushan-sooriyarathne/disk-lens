//! Drags DiskLens.app out of the window, so it can be dropped on the Full Disk Access
//! list (PRD FR-ONB-3a). GPUI's drags stay inside its own window, so this starts an
//! AppKit dragging session from GPUI's content view.

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AllocAnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession,
    NSDraggingSource, NSView, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Side of the icon that follows the pointer.
const ICON: f64 = 64.;

/// The `.app` bundle this process runs from; none for `cargo run`.
pub fn app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
}

/// What Full Disk Access is granted to: the bundle, or under `cargo run` the binary,
/// which the list accepts too.
pub fn app_path() -> io::Result<PathBuf> {
    match app_bundle() {
        Some(bundle) => Ok(bundle),
        None => std::env::current_exe(),
    }
}

/// Selects the app in Finder, where it can be dragged from when the in-app drag does
/// not work.
pub fn reveal_app() -> io::Result<()> {
    // argv, no shell.
    Command::new("/usr/bin/open")
        .arg("-R")
        .arg(app_path()?)
        .status()?;
    Ok(())
}

/// Starts dragging `bundle` from the current mouse event. Call it from a GPUI mouse
/// handler on the main thread.
pub fn drag_app(handle: &impl HasWindowHandle, bundle: &Path) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("dragging needs the main thread")?;
    let raw = handle.window_handle().map_err(|e| e.to_string())?;
    let RawWindowHandle::AppKit(h) = raw.as_raw() else {
        return Err("no AppKit window".into());
    };
    // SAFETY: GPUI hands out the content view of its live window, and this runs on
    // the main thread inside a GPUI callback, so the view is valid for this call.
    let view: &NSView = unsafe { h.ns_view.cast::<NSView>().as_ref() };
    let event = NSApplication::sharedApplication(mtm)
        .currentEvent()
        .ok_or("no mouse event to drag from")?;
    let path = NSString::from_str(&bundle.to_string_lossy());
    let url = NSURL::fileURLWithPath(&path);
    let item = NSDraggingItem::initWithPasteboardWriter(
        NSDraggingItem::alloc(),
        ProtocolObject::from_ref(&*url),
    );
    let icon = NSWorkspace::sharedWorkspace().iconForFile(&path);
    let at = view.convertPoint_fromView(event.locationInWindow(), None);
    let frame = NSRect::new(
        NSPoint::new(at.x - ICON / 2., at.y - ICON / 2.),
        NSSize::new(ICON, ICON),
    );
    let contents: &AnyObject = &icon;
    // SAFETY: an NSImage is a valid dragging image.
    unsafe { item.setDraggingFrame_contents(frame, Some(contents)) };
    let source = DragSource::new(mtm);
    view.beginDraggingSessionWithItems_event_source(
        &NSArray::from_retained_slice(&[item]),
        &event,
        ProtocolObject::from_ref(&*source),
    );
    // The session does not retain its source; keep it until the next drag.
    SOURCE.set(Some(source));
    Ok(())
}

thread_local! {
    static SOURCE: RefCell<Option<Retained<DragSource>>> = const { RefCell::new(None) };
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and DragSource has no Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    struct DragSource;

    // SAFETY: NSObjectProtocol has no safety requirements.
    unsafe impl NSObjectProtocol for DragSource {}

    // SAFETY: the method signature matches NSDraggingSource.
    unsafe impl NSDraggingSource for DragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn operations(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            // System Settings takes the app as a copy or a link; never a move.
            NSDragOperation::Copy | NSDragOperation::Link | NSDragOperation::Generic
        }
    }
);

impl DragSource {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's init is always safe on a fresh allocation.
        unsafe { msg_send![super(this), init] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_binary_is_not_in_a_bundle() {
        assert_eq!(app_bundle(), None);
    }
}
