//! Native UI drawn with GPUI. `AppModel` (model.rs) is the single entity;
//! the other modules are pure logic or render functions over it.

pub mod cleanup;
pub mod explorer;
pub mod format;
pub mod model;
pub mod recommendations;
pub mod root;
pub mod screens;
pub mod selection;
pub mod source;
pub mod stat_cards;
pub mod stats;
pub mod theme;
pub mod treemap;

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

use crate::rules::model::Rules;
use model::AppModel;
use source::Source;

/// Opens the main window and runs the app until it closes.
pub fn run() {
    // Validates the bundled rules once at startup; an invalid file is a build defect.
    Rules::embedded();
    gpui_platform::application().run(|cx: &mut App| {
        root::bind_keys(cx);
        let bounds = Bounds::centered(None, size(px(1280.), px(860.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(1100.), px(720.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("DiskLens".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| AppModel::new(Source::from_env(), window, cx)),
        )
        .expect("open main window");
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        cx.activate(true);
    });
}
