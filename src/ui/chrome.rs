//! Window chrome (PRD 13.5, Window). Every platform difference in the window's
//! frame lives here, so a later Windows or Linux build changes only this file.

use gpui::{
    App, Bounds, TitlebarOptions, WindowBackgroundAppearance, WindowBounds, WindowOptions, point,
    px, size,
};

pub const SIDEBAR_WIDTH: f32 = 248.;
/// Unified toolbar height; the traffic lights sit centred in it.
pub const TOOLBAR_HEIGHT: f32 = 52.;
pub const INSPECTOR_WIDTH: f32 = 320.;

pub fn window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(960.), px(640.))),
        titlebar: Some(TitlebarOptions {
            title: Some("DiskLens".into()),
            // The toolbar is drawn by the app; the titlebar stays draggable natively.
            appears_transparent: true,
            traffic_light_position: Some(point(px(18.), px(18.))),
        }),
        window_background: background(),
        ..Default::default()
    }
}

/// macOS shows the desktop blurred through the sidebar's translucent fill;
/// elsewhere the window is opaque until a platform backdrop is wired up.
fn background() -> WindowBackgroundAppearance {
    if cfg!(target_os = "macos") {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Opaque
    }
}
