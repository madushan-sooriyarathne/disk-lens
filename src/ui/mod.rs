//! Native UI drawn with GPUI. `AppModel` (model.rs) is the single entity;
//! the other modules are pure logic or render functions over it.

pub mod assets;
pub mod chrome;
pub mod cleanup;
pub mod explorer;
pub mod fda_preview;
pub mod filter;
pub mod format;
pub mod inspector;
pub mod keymap;
pub mod model;
pub mod motion;
pub mod onboarding;
pub mod overview;
pub mod palette;
pub mod recommendations;
pub mod root;
pub mod screens;
pub mod selection;
pub mod settings_window;
pub mod shell;
pub mod source;
pub mod stats;
pub mod theme;
pub mod treemap;

use futures::StreamExt;
use gpui::{App, AppContext};

use crate::rules::model::Rules;
use model::AppModel;
use source::Source;

/// Opens the main window and runs the app until it closes.
pub fn run() {
    // Validates the bundled rules once at startup; an invalid file is a build defect.
    Rules::embedded();
    let app = gpui_platform::application().with_assets(assets::Assets);
    // macOS can deliver a link while the app is still starting, before any window exists,
    // so the callback only queues it for a task that starts with the app.
    let (links, mut queued) = futures::channel::mpsc::unbounded();
    app.on_open_urls(move |urls| {
        // The receiver lives as long as the app; a failed send means it is quitting.
        links.unbounded_send(urls).ok();
    });
    app.run(|cx: &mut App| {
        gpui_component::init(cx);
        root::bind_keys(cx);
        root::set_menus(cx);
        let main = cx
            .open_window(chrome::window_options(cx), |window, cx| {
                onboarding::preload(window, cx);
                let model = cx.new(|cx| AppModel::new(Source::from_env(), window, cx));
                cx.set_global(settings_window::MainModel(model.clone()));
                // gpui-component's popovers, tooltips and sheets mount under Root.
                cx.new(|cx| gpui_component::Root::new(model, window, cx))
            })
            .expect("open main window");
        // Settings is a second window; only closing the main one ends the app.
        let main_id = main.window_id();
        cx.on_window_closed(move |cx, closed| {
            if closed == main_id {
                cx.quit();
            }
        })
        .detach();
        cx.spawn(async move |cx| {
            while let Some(urls) = queued.next().await {
                if let Some(token) = settings_window::deep_link_token(&urls) {
                    // Fills the field; activating takes a click.
                    cx.update(|cx| settings_window::open_main(Some(token), cx));
                }
            }
        })
        .detach();
        cx.activate(true);
    });
}
