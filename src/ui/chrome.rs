//! Window chrome (PRD 13.5, Window). Every platform difference in the window's
//! frame lives here, so a later Windows or Linux build changes only this file.

use gpui::{
    App, Bounds, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
    point, px, size,
};

use crate::platform::window::{self, Frame, Moved};

pub const SIDEBAR_WIDTH: f32 = 248.;
/// Unified toolbar height; the traffic lights sit centred in it.
pub const TOOLBAR_HEIGHT: f32 = 52.;
pub const INSPECTOR_WIDTH: f32 = 320.;
const MIN_WIDTH: f32 = 960.;
const MIN_HEIGHT: f32 = 640.;

pub fn window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(MIN_WIDTH), px(MIN_HEIGHT))),
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

/// Moves the window to the right half of its screen, so the onboarding card stays
/// beside System Settings (PRD FR-ONB-3a). The result puts it back.
pub fn dock_right(window: &Window, animate: bool) -> Option<Moved> {
    window::move_window(window, right_half, animate)
}

/// The right half of a screen's visible frame, never below the minimum width.
fn right_half(screen: Frame) -> Frame {
    let width = (screen.width / 2.)
        .max(f64::from(MIN_WIDTH))
        .min(screen.width);
    Frame {
        x: screen.x + screen.width - width,
        y: screen.y,
        width,
        height: screen.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Frame = Frame {
        x: 0.,
        y: 25.,
        width: 2560.,
        height: 1415.,
    };

    #[test]
    fn right_half_takes_the_right_half() {
        assert_eq!(
            right_half(SCREEN),
            Frame {
                x: 1280.,
                y: 25.,
                width: 1280.,
                height: 1415.
            }
        );
    }

    #[test]
    fn right_half_keeps_the_minimum_width_on_a_small_screen() {
        let small = Frame {
            width: 1440.,
            ..SCREEN
        };
        assert_eq!(right_half(small).x, 480.);
    }

    #[test]
    fn right_half_never_leaves_a_screen_narrower_than_the_minimum() {
        let tiny = Frame {
            width: 800.,
            ..SCREEN
        };
        assert_eq!(right_half(tiny).x, 0.);
    }
}
