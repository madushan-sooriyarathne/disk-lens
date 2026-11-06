//! The Settings window (⌘,): one License section (PRD 13.6). It is a second window over the
//! same `AppModel`, which holds the license state; this view only reads it and calls its
//! methods. Also the helpers around it: the site URLs, the status wording and the
//! `disklens://` link routing.

use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, Context, Entity, FontWeight, Global, IntoElement,
    Render, Subscription, TitlebarOptions, WeakEntity, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, size,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::switch::Switch;
use gpui_component::{Disableable, Sizable, Size};

use super::model::AppModel;
use super::theme;
use crate::cleanup::audit::now_ms;
use crate::license::{self, Kind, Status};

/// The marketing site (`site/`); Buy and Start Trial open pages on it.
const SITE_URL: &str = "https://disklens.space";
const WIDTH: f32 = 520.;
const HEIGHT: f32 = 350.;
const SECONDS_PER_DAY: u64 = 86_400;

/// The main window's model, for the menu action and `disklens://` links, which arrive
/// without a window.
pub struct MainModel(pub Entity<AppModel>);

impl Global for MainModel {}

/// The open Settings window, so a second request focuses it.
struct Open {
    window: AnyWindowHandle,
    view: WeakEntity<SettingsView>,
}

impl Global for Open {}

/// `<site>/<path>`; `DISKLENS_SITE_URL` points it at a local or staging site.
pub fn site_url(path: &str) -> String {
    join(
        &std::env::var("DISKLENS_SITE_URL").unwrap_or_else(|_| SITE_URL.into()),
        path,
    )
}

fn join(base: &str, path: &str) -> String {
    format!("{}/{path}", base.trim_end_matches('/'))
}

/// The token in the first URL that is a `disklens://activate` link; anything else is ignored.
pub fn deep_link_token(urls: &[String]) -> Option<String> {
    urls.iter().find_map(|u| license::parse_deep_link(u))
}

/// "DiskLens Pro", "Trial: 5 days left", "Trial ended", "Free", or why a stored license
/// does not work. `now` is unix seconds.
pub fn status_line(status: &Status, now: u64) -> String {
    match status {
        Status::Free => "Free".into(),
        Status::Pro { .. } => "DiskLens Pro".into(),
        Status::Trial { ends, .. } => {
            // Rounded up, so the last day still reads "1 day left".
            let days = ends.saturating_sub(now).div_ceil(SECONDS_PER_DAY).max(1);
            let noun = if days == 1 { "day" } else { "days" };
            format!("Trial: {days} {noun} left")
        }
        Status::Ended {
            kind: Kind::Trial, ..
        } => "Trial ended".into(),
        Status::Ended { .. } => "License ended".into(),
        Status::Invalid(e) => capitalize(&e.to_string()),
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// Opens Settings, or brings the open one forward. A `token` fills the field; activating
/// it always takes a click.
pub fn open(model: &Entity<AppModel>, token: Option<String>, cx: &mut App) {
    cx.activate(true);
    let existing = cx.try_global::<Open>().map(|o| (o.window, o.view.clone()));
    if let Some((window, view)) = existing {
        let shown = window.update(cx, |_, window, cx| {
            window.activate_window();
            if let (Some(token), Some(view)) = (&token, view.upgrade()) {
                view.update(cx, |v, cx| v.set_token(token, window, cx));
            }
        });
        if shown.is_ok() {
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Settings".into()),
            ..Default::default()
        }),
        is_resizable: false,
        is_minimizable: false,
        ..Default::default()
    };
    let model = model.clone();
    let opened = cx.open_window(options, |window, cx| {
        theme::sync_components(window, cx);
        let view = cx.new(|cx| SettingsView::new(model, token, window, cx));
        cx.set_global(Open {
            window: window.window_handle(),
            view: view.downgrade(),
        });
        cx.new(|cx| gpui_component::Root::new(view, window, cx))
    });
    if let Err(e) = opened {
        tracing::warn!("could not open Settings: {e}");
    }
}

/// `open` for callers without the model at hand; does nothing before the main window exists.
pub fn open_main(token: Option<String>, cx: &mut App) {
    if let Some(MainModel(model)) = cx.try_global::<MainModel>() {
        let model = model.clone();
        open(&model, token, cx);
    }
}

struct SettingsView {
    model: Entity<AppModel>,
    token: Entity<InputState>,
    _model: Subscription,
    _input: Subscription,
}

impl SettingsView {
    fn new(
        model: Entity<AppModel>,
        token: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Paste your license key"));
        let _input = cx.subscribe_in(&input, window, |this, _, event, _, cx| match event {
            InputEvent::Change => {
                // A new attempt starts clean.
                this.model.update(cx, |m, cx| {
                    if m.license_error.take().is_some() {
                        cx.notify();
                    }
                });
                cx.notify();
            }
            InputEvent::PressEnter { .. } => this.activate(cx),
            _ => {}
        });
        let _model = cx.observe(&model, |_, _, cx| cx.notify());
        let mut view = Self {
            model,
            token: input,
            _model,
            _input,
        };
        if let Some(token) = token {
            view.set_token(&token, window, cx);
        }
        view
    }

    fn set_token(&mut self, token: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.token
            .update(cx, |s, cx| s.set_value(token.to_owned(), window, cx));
        self.token.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    fn activate(&mut self, cx: &mut Context<Self>) {
        let token = self.token.read(cx).value().trim().to_owned();
        if !token.is_empty() {
            self.model.update(cx, |m, cx| m.activate_license(token, cx));
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current(window);
        let model = self.model.read(cx);
        let status = model.license.clone();
        let error = model.license_error.clone();
        let line = status_line(&status, now_ms() / 1000);
        let has_token = !matches!(status, Status::Free);
        let empty = self.token.read(cx).value().trim().is_empty();
        let problem = matches!(status, Status::Invalid(_));
        let allow_admin = model.allow_admin_actions;

        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .bg(t.content)
            .text_color(t.label)
            .font_family(theme::SANS)
            .text_size(px(theme::T_BODY))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(theme::T_SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.secondary)
                            .child("License"),
                    )
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if problem { t.review_text } else { t.label })
                            .child(line),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(Input::new(&self.token)))
                    .child(
                        Button::new("activate")
                            .primary()
                            .with_size(Size::Small)
                            .label("Activate")
                            .disabled(empty)
                            .on_click(cx.listener(|this, _, _, cx| this.activate(cx))),
                    ),
            )
            .children(error.map(|e| {
                div()
                    .text_size(px(12.))
                    .text_color(t.danger)
                    .child(capitalize(&e))
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children((!status.is_pro()).then(|| {
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("buy")
                                    .with_size(Size::Small)
                                    .label("Buy DiskLens Pro")
                                    .on_click(|_, _, cx| cx.open_url(&site_url("buy"))),
                            )
                            .child(
                                Button::new("trial")
                                    .with_size(Size::Small)
                                    .label("Start 7-Day Trial")
                                    .on_click(|_, _, cx| cx.open_url(&site_url("trial"))),
                            )
                    }))
                    .child(div().flex_1())
                    .children(has_token.then(|| {
                        Button::new("remove")
                            .with_size(Size::Small)
                            .label("Remove License")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.model.update(cx, |m, cx| m.remove_license(cx));
                            }))
                    })),
            )
            // Only snapshot actions use it, and they are Pro (PRD FR-SNAP-5).
            .children(status.is_pro().then(|| {
                let model = self.model.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .pt_2()
                    .child(
                        Switch::new("allow-admin")
                            .label("Allow actions that need administrator rights")
                            .checked(allow_admin)
                            .on_click(move |on, _, cx| {
                                model.update(cx, |m, cx| m.set_allow_admin_actions(*on, cx));
                            }),
                    )
                    .child(div().text_size(px(12.)).text_color(t.secondary).child(
                        "Used only to delete or thin local Time Machine snapshots. macOS \
                                 asks for your password each time, and nothing is kept.",
                    ))
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::LicenseError;

    const DAY: u64 = SECONDS_PER_DAY;

    fn trial(ends: u64) -> Status {
        Status::Trial {
            id: "t".into(),
            ends,
        }
    }

    #[test]
    fn a_trial_counts_whole_days_up() {
        assert_eq!(status_line(&trial(DAY + 1), 0), "Trial: 2 days left");
    }

    #[test]
    fn the_last_day_is_singular() {
        assert_eq!(status_line(&trial(60), 0), "Trial: 1 day left");
    }

    #[test]
    fn a_pro_license_reads_disklens_pro() {
        assert_eq!(
            status_line(&Status::Pro { id: "p".into() }, 0),
            "DiskLens Pro"
        );
    }

    #[test]
    fn an_ended_trial_says_so() {
        let ended = Status::Ended {
            kind: Kind::Trial,
            id: "t".into(),
        };
        assert_eq!(status_line(&ended, 0), "Trial ended");
    }

    #[test]
    fn an_invalid_license_gives_its_reason() {
        assert_eq!(
            status_line(&Status::Invalid(LicenseError::EarlierMajorVersion), 0),
            "This license is for an earlier major version of DiskLens"
        );
    }

    #[test]
    fn a_free_install_reads_free() {
        assert_eq!(status_line(&Status::Free, 0), "Free");
    }

    #[test]
    fn the_site_url_drops_a_trailing_slash() {
        assert_eq!(
            join("http://localhost:3000/", "buy"),
            "http://localhost:3000/buy"
        );
    }

    #[test]
    fn an_activation_link_gives_its_token() {
        let urls = ["disklens://activate?token=DL1.a.b".to_owned()];
        assert_eq!(deep_link_token(&urls).as_deref(), Some("DL1.a.b"));
    }

    #[test]
    fn a_link_that_is_not_an_activation_gives_nothing() {
        let urls = [
            "disklens://activate?token=evil".to_owned(),
            "https://example.com/?token=DL1.a.b".to_owned(),
        ];
        assert_eq!(deep_link_token(&urls), None);
    }
}
