//! The Local Snapshots sheet (Pro, PRD 13.10.5): local Time Machine snapshots with their
//! age, the other APFS snapshots read-only, the purgeable total, Thin and Delete. Every
//! action first shows the exact command; one that needs administrator rights first needs
//! the opt-in (FR-SNAP-5).

use gpui::{AnyElement, Context, Div, FontWeight, Window, deferred, div, prelude::*, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::switch::Switch;
use gpui_component::{Disableable, Sizable, Size};

use super::format::{format_age, format_bytes};
use super::model::AppModel;
use super::theme::{self, Theme};
use crate::backend::SnapshotInfo;
use crate::cleanup::audit::now_ms;
use crate::cleanup::snapshots::{SnapshotAction, SnapshotOutcome};

/// The sheet's state while it is open.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotsView {
    /// `None` while the list loads.
    pub info: Option<SnapshotInfo>,
    /// Why the list could not be read.
    pub error: Option<String>,
    /// The action waiting for Confirm.
    pub confirm: Option<SnapshotAction>,
    pub running: bool,
    /// What the last action did.
    pub message: Option<String>,
    /// `tmutil` failed without administrator rights: the confirm offers the opt-in, then
    /// the retry through the prompt.
    pub needs_admin: bool,
}

/// "Up to 38.0 GB is purgeable, mostly held by local snapshots." (FR-SNAP-2).
pub fn purgeable_line(bytes: u64) -> String {
    format!(
        "Up to {} is purgeable, mostly held by local snapshots.",
        format_bytes(bytes)
    )
}

/// `2026-11-06-091500` as `2026-11-06 09:15`. The date has passed `is_snapshot_date`.
pub fn date_label(date: &str) -> String {
    format!("{} {}:{}", &date[..10], &date[11..13], &date[13..15])
}

/// The result line after an action.
pub fn outcome_line(action: &SnapshotAction, outcome: &SnapshotOutcome) -> String {
    match (outcome, action) {
        (SnapshotOutcome::DryRun, _) => {
            "Dry run: the command was checked and logged, nothing ran.".into()
        }
        (SnapshotOutcome::Ran, SnapshotAction::Delete { date }) => {
            format!("Deleted the snapshot from {}.", date_label(date))
        }
        (SnapshotOutcome::Ran, SnapshotAction::Thin { bytes }) => format!(
            "Asked macOS to free up to {} held by local snapshots.",
            format_bytes(*bytes)
        ),
        (SnapshotOutcome::NeedsAdmin, _) => {
            "macOS refused without administrator rights. Nothing changed.".into()
        }
        (SnapshotOutcome::Failed(why), _) => format!("Nothing changed: {why}."),
    }
}

pub fn render(
    model: &AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let view = model.snapshots.as_ref()?;
    let t = theme::current(window);
    let busy = view.running || view.confirm.is_some();
    let now = i64::try_from(now_ms() / 1000).unwrap_or(i64::MAX);

    let mut body = div().flex().flex_col().gap_3().child(
        div()
            .text_size(px(17.))
            .font_weight(FontWeight::SEMIBOLD)
            .child("Local Snapshots"),
    );
    match (&view.info, &view.error) {
        (_, Some(e)) => body = body.child(div().text_color(t.danger).child(e.clone())),
        (None, None) => body = body.child(muted(&t, "Reading snapshots…")),
        (Some(info), None) => {
            if let Some(bytes) = info.purgeable {
                body = body.child(muted(&t, purgeable_line(bytes)));
            }
            body = body.child(local_list(&t, info, now, busy, cx));
            if !info.others.is_empty() {
                body = body.child(others(&t, &info.others));
            }
        }
    }
    if let Some(action) = &view.confirm {
        body = body.child(confirm(
            &t,
            action,
            view.needs_admin,
            model.allow_admin_actions,
            model.source.is_dry_run(),
            view.running,
            cx,
        ));
    }
    if let Some(message) = &view.message {
        body = body.child(note(&t).child(message.clone()));
    }
    let thin = view
        .info
        .as_ref()
        .and_then(|i| i.purgeable.filter(|b| *b > 0 && !i.local.is_empty()));
    let footer = div()
        .flex()
        .items_center()
        .gap_2()
        .children(thin.map(|bytes| {
            Button::new("snapshots-thin")
                .with_size(Size::Small)
                .label("Thin Snapshots…")
                .disabled(busy)
                .on_click(cx.listener(move |m, _, _, cx| {
                    m.ask_snapshot_action(SnapshotAction::Thin { bytes }, cx)
                }))
        }))
        .child(div().flex_1())
        .child(
            Button::new("snapshots-close")
                .with_size(Size::Small)
                .label("Close")
                .disabled(view.running)
                .on_click(cx.listener(|m, _, _, cx| m.close_snapshots(cx))),
        );

    Some(
        deferred(
            div()
                .id("snapshots-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .bg(t.scrim)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .id("snapshots")
                        .w(px(520.))
                        .max_h(gpui::relative(0.85))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .p_5()
                        .rounded(px(theme::R_WIDGET))
                        .bg(t.popover)
                        .shadow_lg()
                        .text_size(px(theme::T_BODY))
                        .text_color(t.label)
                        .child(body)
                        .child(footer),
                ),
        )
        .with_priority(2)
        .into_any_element(),
    )
}

fn local_list(
    t: &Theme,
    info: &SnapshotInfo,
    now: i64,
    busy: bool,
    cx: &mut Context<AppModel>,
) -> Div {
    if info.local.is_empty() {
        return muted(t, "No local Time Machine snapshots.");
    }
    div()
        .flex()
        .flex_col()
        .children(info.local.iter().enumerate().map(|(i, snap)| {
            let then = i64::try_from(snap.epoch_secs()).unwrap_or(0);
            let date = snap.date.clone();
            div()
                .flex()
                .items_center()
                .gap_3()
                .py_1()
                .when(i > 0, |d| d.border_t_1().border_color(t.separator))
                .child(
                    div()
                        .flex_1()
                        .font_features(theme::numeric())
                        .child(date_label(&snap.date)),
                )
                .child(muted(t, format_age(now, then)))
                .child(
                    Button::new(("snapshot-delete", i))
                        .with_size(Size::Small)
                        .label("Delete…")
                        .disabled(busy)
                        .on_click(cx.listener(move |m, _, _, cx| {
                            m.ask_snapshot_action(SnapshotAction::Delete { date: date.clone() }, cx)
                        })),
                )
        }))
}

/// Other APFS snapshots: listed so nothing is hidden, never offered (S-12).
fn others(t: &Theme, names: &[String]) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(theme::T_SMALL))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.secondary)
                .child("Managed by macOS"),
        )
        .children(names.iter().map(|n| muted(t, n.clone())))
}

/// The exact command and Confirm. After `tmutil` failed without administrator rights, the
/// opt-in first, then the retry through the system prompt.
fn confirm(
    t: &Theme,
    action: &SnapshotAction,
    admin: bool,
    allow_admin: bool,
    dry_run: bool,
    running: bool,
    cx: &mut Context<AppModel>,
) -> Div {
    let mut c = note(t).flex().flex_col().gap_2();
    if admin && !allow_admin {
        c = c
            .child(
                "macOS refused this without administrator rights. DiskLens can retry it once \
                 through the system password prompt, and keeps nothing afterwards. Only snapshot \
                 actions ever use these rights.",
            )
            .child(
                Switch::new("snapshots-allow-admin")
                    .label("Allow actions that need administrator rights")
                    .checked(false)
                    .on_click(
                        cx.listener(|m, on: &bool, _, cx| m.set_allow_admin_actions(*on, cx)),
                    ),
            );
    } else {
        c = c
            .child("This runs:")
            .child(
                div()
                    .font_family(theme::MONO)
                    .text_size(px(12.))
                    .child(action.tmutil_argv().join(" ")),
            )
            .when(admin, |d| {
                d.child(muted(
                    t,
                    "macOS asks for an administrator password next, through osascript. The authorization is not kept.",
                ))
            })
            .when(dry_run, |d| {
                d.child(muted(
                    t,
                    "Dry run: the command is checked and logged, not run.",
                ))
            });
    }
    let ready = !admin || allow_admin;
    c.child(
        div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                Button::new("snapshots-cancel")
                    .with_size(Size::Small)
                    .label("Cancel")
                    .disabled(running)
                    .on_click(cx.listener(|m, _, _, cx| m.cancel_snapshot_action(cx))),
            )
            .children(ready.then(|| {
                Button::new("snapshots-confirm")
                    .with_size(Size::Small)
                    .primary()
                    .label(if running { "Running…" } else { "Confirm" })
                    .disabled(running)
                    .on_click(cx.listener(|m, _, _, cx| m.confirm_snapshot_action(cx)))
            })),
    )
}

fn muted(t: &Theme, text: impl Into<String>) -> Div {
    div()
        .text_size(px(12.))
        .line_height(px(17.))
        .text_color(t.secondary)
        .child(text.into())
}

fn note(t: &Theme) -> Div {
    div()
        .px_3()
        .py(px(10.))
        .rounded(px(theme::R_CONTROL))
        .bg(t.widget)
        .text_size(px(12.))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_purgeable_line_names_the_total() {
        assert_eq!(
            purgeable_line(38_000_000_000),
            "Up to 38.0 GB is purgeable, mostly held by local snapshots."
        );
    }

    #[test]
    fn a_snapshot_date_reads_as_day_and_time() {
        assert_eq!(date_label("2026-11-06-091500"), "2026-11-06 09:15");
    }

    #[test]
    fn a_deleted_snapshot_is_named_by_its_date() {
        let action = SnapshotAction::Delete {
            date: "2026-11-06-091500".into(),
        };
        assert_eq!(
            outcome_line(&action, &SnapshotOutcome::Ran),
            "Deleted the snapshot from 2026-11-06 09:15."
        );
    }

    #[test]
    fn a_failure_says_nothing_changed() {
        let action = SnapshotAction::Thin { bytes: 1 };
        assert_eq!(
            outcome_line(
                &action,
                &SnapshotOutcome::Failed("Cancelled or failed".into())
            ),
            "Nothing changed: Cancelled or failed."
        );
    }
}
