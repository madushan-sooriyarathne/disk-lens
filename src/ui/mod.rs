//! Native UI drawn with GPUI. `AppModel` (model.rs) is the single entity;
//! the other modules are pure logic or render functions over it.

pub mod assets;
pub mod chrome;
pub mod cleanup;
pub mod explorer;
pub mod format;
pub mod model;
pub mod recommendations;
pub mod root;
pub mod screens;
pub mod selection;
pub mod shell;
pub mod source;
pub mod stat_cards;
pub mod stats;
pub mod theme;
pub mod treemap;

use gpui::{App, AppContext};

use crate::rules::model::Rules;
use model::AppModel;
use source::Source;

/// Opens the main window and runs the app until it closes.
pub fn run() {
    // Validates the bundled rules once at startup; an invalid file is a build defect.
    Rules::embedded();
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(|cx: &mut App| {
            gpui_component::init(cx);
            root::bind_keys(cx);
            root::set_menus(cx);
            cx.open_window(chrome::window_options(cx), |window, cx| {
                let model = cx.new(|cx| AppModel::new(Source::from_env(), window, cx));
                // gpui-component's popovers, tooltips and sheets mount under Root.
                cx.new(|cx| gpui_component::Root::new(model, window, cx))
            })
            .expect("open main window");
            cx.on_window_closed(|cx, _| cx.quit()).detach();
            cx.activate(true);
        });
}
