//! Quick Look (PRD FR-EXP-8, ⌘Y) through the shared `QLPreviewPanel`.

use std::cell::RefCell;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send, sel};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString, NSURL};
use objc2_quick_look_ui::{QLPreviewItem, QLPreviewPanel, QLPreviewPanelDataSource};

/// Toggles the panel for `path`. The caller passes a path the backend has validated
/// against the snapshot. Must run on the main thread.
pub fn quick_look(path: &Path) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("Quick Look needs the main thread")?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let same = PREVIEW.with_borrow(|s| s.as_ref().is_some_and(|s| *s.ivars().url == *url));
    // SAFETY: called on the main thread, as the marker proves.
    let visible = unsafe { QLPreviewPanel::sharedPreviewPanelExists(mtm) }
        && unsafe { QLPreviewPanel::sharedPreviewPanel(mtm) }.is_some_and(|p| p.isVisible());
    let source = PreviewSource::new(url);
    // Showing the panel makes it key, and GPUI handles our window resigning key by
    // borrowing the app, which the caller (an action handler) already holds. So the
    // panel calls run on the next turn of the main run loop instead.
    let selector = if same && visible {
        sel!(hide)
    } else {
        sel!(show)
    };
    // SAFETY: `source` implements both selectors and takes no argument; the object is
    // retained by PREVIEW and by the run loop until the call runs.
    unsafe {
        let _: () = msg_send![
            &*source,
            performSelectorOnMainThread: selector,
            withObject: None::<&AnyObject>,
            waitUntilDone: false
        ];
    }
    PREVIEW.set(Some(source));
    Ok(())
}

thread_local! {
    static PREVIEW: RefCell<Option<Retained<PreviewSource>>> = const { RefCell::new(None) };
}

struct PreviewIvars {
    url: Retained<NSURL>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and PreviewSource has no Drop.
    #[unsafe(super = NSObject)]
    #[ivars = PreviewIvars]
    struct PreviewSource;

    // SAFETY: NSObjectProtocol has no safety requirements.
    unsafe impl NSObjectProtocol for PreviewSource {}

    impl PreviewSource {
        #[unsafe(method(show))]
        fn show(&self) {
            let Some(panel) = shared_panel() else { return };
            // SAFETY: the panel keeps its data source unretained; PREVIEW keeps this one
            // alive until a later call hands the panel a new one.
            unsafe {
                panel.setDataSource(Some(ProtocolObject::from_ref(self)));
                panel.reloadData();
            }
            panel.makeKeyAndOrderFront(None);
        }

        #[unsafe(method(hide))]
        fn hide(&self) {
            if let Some(panel) = shared_panel() {
                panel.orderOut(None);
            }
        }
    }

    // SAFETY: both method signatures match QLPreviewPanelDataSource.
    unsafe impl QLPreviewPanelDataSource for PreviewSource {
        #[unsafe(method(numberOfPreviewItemsInPreviewPanel:))]
        fn number_of_items(&self, _panel: Option<&QLPreviewPanel>) -> isize {
            1
        }

        #[unsafe(method_id(previewPanel:previewItemAtIndex:))]
        fn item(
            &self,
            _panel: Option<&QLPreviewPanel>,
            _index: isize,
        ) -> Option<Retained<ProtocolObject<dyn QLPreviewItem>>> {
            Some(ProtocolObject::from_retained(self.ivars().url.clone()))
        }
    }
);

fn shared_panel() -> Option<Retained<QLPreviewPanel>> {
    let mtm = MainThreadMarker::new()?;
    // SAFETY: called on the main thread, as the marker proves.
    unsafe { QLPreviewPanel::sharedPreviewPanel(mtm) }
}

impl PreviewSource {
    fn new(url: Retained<NSURL>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(PreviewIvars { url });
        // SAFETY: NSObject's init is always safe on a fresh allocation.
        unsafe { msg_send![super(this), init] }
    }
}
