//! Cleanup sheet (PRD 13.4.8): confirm, progress and results in one sheet that
//! slides down from the toolbar and back up, plus the toast. The confirm button
//! only reacts to a click; no key binding triggers it (PRD 7.6).

use gpui::{
    Animation, AnimationExt as _, AnyElement, ClickEvent, Context, Div, FontWeight, Role, Window,
    deferred, div, prelude::*, px, relative,
};

use super::chrome::{SIDEBAR_WIDTH, TOOLBAR_HEIGHT};
use super::format::{format_bytes, format_size};
use super::model::{AppModel, Cleanup};
use super::motion;
use super::overview::PURGEABLE_MIN;
use super::recommendations::{checkbox, toggled};
use super::selection::RowState;
use super::theme::{self, Theme};
use crate::cleanup::model::{CleanupPlan, CleanupSummary, PlannedAction, PlannedMethod};

const WIDTH: f32 = 600.;
/// How far the sheet travels down out of the toolbar; the rest of it starts hidden behind it.
const SLIDE: f32 = 480.;

/// The sheet for the model's cleanup stage, or the one sliding away after a close.
pub fn render(
    model: &AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let t = theme::current(window);
    let (stage, leaving) = match (&model.cleanup, &model.sheet_exit) {
        (Some(stage), _) => (stage, false),
        (None, Some(stage)) => (stage, true),
        (None, None) => return None,
    };
    let dry_run = model.source.is_dry_run();
    let sheet = match stage {
        Cleanup::Planning => sheet(&t).child(muted(&t, "Preparing the cleanup…")),
        Cleanup::Confirm { plan, permanent } => confirm(&t, plan, *permanent, dry_run, cx),
        Cleanup::Running {
            plan,
            done,
            current,
        } => running(&t, plan, *done, current.as_deref()),
        Cleanup::Results { summary, held } => results(&t, summary, *held, model.scan.is_some(), cx),
    };

    let transition = motion::Transition::new(motion::NAV, model.reduce_motion);
    let moves = matches!(transition, motion::Transition::Move { .. });
    let animation = Animation::new(transition.duration()).with_easing(motion::ease);
    let id = (
        if leaving { "sheet-out" } else { "sheet-in" },
        model.sheet_seq,
    );
    // Enter and exit share one path: down out of the toolbar, and back up into it.
    let shown = move |p: f32| if leaving { 1. - p } else { p };
    let scrim = div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(t.scrim)
        .with_animation(id, animation.clone(), move |d, p| d.opacity(shown(p)));
    let sheet = sheet.with_animation(id, animation, move |d, p| {
        if moves {
            d.top(px(-(1. - shown(p)) * SLIDE))
        } else {
            d.opacity(shown(p))
        }
    });
    Some(
        deferred(
            div()
                .id("cleanup-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .child(scrim)
                .child(
                    // Under the toolbar, over the content area; clips the sheet while it
                    // is still behind the toolbar.
                    div()
                        .absolute()
                        .top(px(TOOLBAR_HEIGHT))
                        .bottom_0()
                        .left(px(if model.sidebar_visible {
                            SIDEBAR_WIDTH
                        } else {
                            0.
                        }))
                        .right_0()
                        .overflow_hidden()
                        .flex()
                        .justify_center()
                        .child(sheet),
                ),
        )
        .priority(2)
        .into_any_element(),
    )
}

/// The toast at the bottom of the window.
pub fn toast(model: &AppModel, window: &mut Window) -> Option<AnyElement> {
    let t = theme::current(window);
    let text = model.toast.clone()?;
    Some(
        deferred(
            div()
                .absolute()
                .bottom(px(24.))
                .left_0()
                .w_full()
                .flex()
                .justify_center()
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .rounded_lg()
                        .bg(t.label)
                        .text_color(t.widget)
                        .text_size(px(13.))
                        .child(text),
                ),
        )
        .priority(3)
        .into_any_element(),
    )
}

/// `Clean 52.5 GB` (PRD section 7.6, step 5).
fn confirm_label(plan: &CleanupPlan) -> String {
    format!("Clean {}", format_bytes(plan.total))
}

/// Actions grouped by row label, in plan order.
fn groups(plan: &CleanupPlan) -> Vec<(&str, Vec<&PlannedAction>)> {
    let mut out: Vec<(&str, Vec<&PlannedAction>)> = Vec::new();
    for a in &plan.actions {
        match out.iter_mut().find(|(label, _)| *label == a.label) {
            Some((_, list)) => list.push(a),
            None => out.push((a.label.as_str(), vec![a])),
        }
    }
    out
}

/// The method in plain words; a command shows its program only, since the sheet
/// lists rows, not paths.
fn method_words(m: &PlannedMethod) -> String {
    match m {
        PlannedMethod::Delete => "Deleted permanently".into(),
        PlannedMethod::DeleteContents => "Contents deleted permanently".into(),
        PlannedMethod::Trash => "Moved to Trash".into(),
        PlannedMethod::TrashContents => "Contents moved to Trash".into(),
        PlannedMethod::Command { argv } => {
            let program = argv
                .first()
                .and_then(|p| p.rsplit('/').next())
                .unwrap_or_default();
            format!("Runs {program}")
        }
    }
}

/// Freed bytes that local snapshots still hold, and how many snapshots there are: what
/// was freed beyond what free space grew by, shown from 1 GB (PRD 13.4.8, US-36).
pub(super) fn held_by_snapshots(
    free_before: Option<u64>,
    free_after: Option<u64>,
    freed: u64,
    snapshots: usize,
) -> Option<(u64, usize)> {
    let grew = free_after?.saturating_sub(free_before?);
    let held = freed.saturating_sub(grew);
    (snapshots > 0 && held >= PURGEABLE_MIN).then_some((held, snapshots))
}

fn held_line(held: u64, snapshots: usize) -> String {
    format!(
        "{} is still held by {snapshots} local Time Machine {}.",
        format_bytes(held),
        if snapshots == 1 {
            "snapshot"
        } else {
            "snapshots"
        }
    )
}

fn sheet(t: &Theme) -> Div {
    div()
        .relative()
        .w(px(WIDTH))
        .max_h(relative(0.9))
        .flex()
        .flex_col()
        .gap(px(14.))
        .px_6()
        .pt_5()
        .pb_5()
        .rounded_b(px(theme::R_WIDGET))
        .bg(t.popover)
        .shadow_lg()
        .text_color(t.label)
        .text_size(px(theme::T_BODY))
}

fn heading(text: String) -> Div {
    div()
        .text_size(px(17.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

fn muted(t: &Theme, text: impl Into<String>) -> Div {
    div()
        .text_size(px(12.))
        .line_height(px(17.))
        .text_color(t.secondary)
        .child(text.into())
}

/// A note on the widget fill, like the mockup's.
fn note(t: &Theme, text: String) -> Div {
    muted(t, text)
        .px_3()
        .py(px(10.))
        .rounded(px(theme::R_CONTROL))
        .bg(t.widget)
}

/// `primary` is the green Clean button; `link` a borderless one.
fn button(t: &Theme, id: &'static str, label: String, primary: bool) -> gpui::Stateful<Div> {
    // Not a Tab stop: GPUI clicks a focused element on Enter, and Enter must
    // never confirm a cleanup (PRD 7.6). Cancel and Done opt in at their call sites.
    let b = div()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .px_3()
        .h(px(28.))
        .flex()
        .items_center()
        .rounded(px(6.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .child(label);
    if primary {
        b.bg(t.clean).text_color(t.clean_text)
    } else {
        b.border_1().border_color(t.separator).bg(t.content)
    }
}

fn confirm(
    t: &Theme,
    plan: &CleanupPlan,
    permanent: bool,
    dry_run: bool,
    cx: &mut Context<AppModel>,
) -> Div {
    let count = plan.actions.len();
    let mut list = div()
        .id("cleanup-actions")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll();
    for (i, (label, actions)) in groups(plan).into_iter().enumerate() {
        let size: u64 = actions.iter().map(|a| a.size).sum();
        let estimate = actions.iter().any(|a| a.size_is_estimate);
        let mut methods: Vec<String> = Vec::new();
        for a in &actions {
            let m = method_words(&a.method);
            if !methods.contains(&m) {
                methods.push(m);
            }
        }
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .py(px(7.))
                .when(i > 0, |d| d.border_t_1().border_color(t.separator))
                .child(div().flex_1().min_w_0().truncate().child(label.to_owned()))
                .child(
                    div()
                        .w(px(150.))
                        .flex_none()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(t.secondary)
                        .child(methods.join(", ")),
                )
                .child(
                    div()
                        .w(px(72.))
                        .flex_none()
                        .flex()
                        .justify_end()
                        .font_weight(FontWeight::MEDIUM)
                        .child(format_size(size, estimate)),
                ),
        );
    }
    let mut c = sheet(t)
        .child(heading(format!(
            "Clean up {count} {}?",
            if count == 1 { "item" } else { "items" }
        )))
        .child(list);
    for w in &plan.warnings {
        c = c.child(muted(t, format!("Left out: {w}")));
    }
    let mut lines = Vec::new();
    if plan.trash_total > 0 {
        lines.push(format!(
            "{} goes to the Trash. That space is freed when you empty the Trash.",
            format_bytes(plan.trash_total)
        ));
    }
    lines.push(
        "Every item is checked again right before it is removed, and anything that changed \
         since the scan is skipped."
            .to_owned(),
    );
    if dry_run {
        lines.push("Dry run: every step is checked and logged, nothing is removed.".to_owned());
    }
    c = c.child(note(t, lines.join(" ")));
    if plan.permanent_available {
        let state = if permanent {
            RowState::All
        } else {
            RowState::None
        };
        c = c.child(
            theme::focusable(
                div().id("cleanup-permanent"),
                t,
                Role::CheckBox,
                "Delete permanently instead of moving to Trash",
            )
            .aria_toggled(toggled(state))
            .flex()
            .items_center()
            .gap_2()
            .text_size(px(12.))
            .cursor_pointer()
            .child(checkbox(t, "cleanup-permanent-box", state, false, 16.))
            .child("Delete permanently instead of moving to Trash")
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.set_permanent(!permanent, cx);
            })),
        );
    }
    let mut clean = button(t, "cleanup-confirm", confirm_label(plan), true);
    clean = if count == 0 {
        clean.opacity(0.4)
    } else {
        clean.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.confirm_cleanup(cx)))
    };
    let copy = theme::focusable(
        div().id("cleanup-copy"),
        t,
        Role::Button,
        "Copy as Shell Script",
    )
    .mr_auto()
    .text_color(t.accent_text)
    .cursor_pointer()
    .child("Copy as Shell Script")
    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.copy_script(cx)));
    c.child(
        div()
            .flex()
            .items_center()
            .justify_end()
            .gap_2()
            .child(copy)
            .child(
                theme::focusable(
                    button(t, "cleanup-cancel", "Cancel".into(), false),
                    t,
                    Role::Button,
                    "Cancel",
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_cleanup(cx))),
            )
            .child(clean),
    )
}

fn running(t: &Theme, plan: &CleanupPlan, done: usize, current: Option<&str>) -> Div {
    let total = plan.actions.len().max(1);
    let name = current
        .and_then(|id| plan.actions.iter().find(|a| a.item_id == id))
        .map_or_else(String::new, |a| a.name.clone());
    sheet(t)
        .child(heading(format!(
            "Cleaning… {done} of {}",
            plan.actions.len()
        )))
        .child(muted(t, name))
        .child(
            div()
                .h(px(6.))
                .w_full()
                .rounded_full()
                .bg(t.separator)
                .child(
                    div()
                        .h_full()
                        .rounded_full()
                        .bg(t.accent)
                        .w(relative(done as f32 / total as f32)),
                ),
        )
}

fn results(
    t: &Theme,
    s: &CleanupSummary,
    held: Option<(u64, usize)>,
    updating: bool,
    cx: &mut Context<AppModel>,
) -> Div {
    let mut c = sheet(t).child(heading(if s.dry_run {
        "Dry run finished".into()
    } else {
        "Cleanup finished".into()
    }));
    let (freed, trashed) = if s.dry_run {
        ("Would free", "Would move to Trash")
    } else {
        ("Freed", "Moved to Trash")
    };
    let line = |label: &str, value: u64| {
        div()
            .flex()
            .justify_between()
            .child(label.to_owned())
            .child(
                div()
                    .font_features(theme::numeric())
                    .child(format_bytes(value)),
            )
    };
    c = c.child(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(line(freed, s.freed))
            .child(line(trashed, s.trashed)),
    );
    if let Some((bytes, snapshots)) = held {
        c = c.child(note(t, held_line(bytes, snapshots)));
    }
    if !s.skipped.is_empty() {
        c = c.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .text_size(px(12.))
                .child(muted(t, format!("Skipped {}:", s.skipped.len())))
                .children(s.skipped.iter().map(|item| {
                    div()
                        .text_color(t.danger)
                        .child(format!("{}: {}", item.name, item.reason))
                })),
        );
    }
    c.child(
        div()
            .flex()
            .items_center()
            .gap_2()
            .when(updating, |d| d.child(muted(t, "Updating results…")))
            .child(div().flex_1())
            .child(
                theme::focusable(
                    button(t, "cleanup-done", "Done".into(), false),
                    t,
                    Role::Button,
                    "Done",
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_cleanup(cx))),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recommend::model::Tier;

    fn action(label: &str, size: u64, method: PlannedMethod) -> PlannedAction {
        PlannedAction {
            item_id: format!("{label}:{size}"),
            rule_id: label.into(),
            label: label.into(),
            name: label.into(),
            tier: Tier::Safe,
            path: format!("/Users/x/{label}"),
            matched: format!("/Users/x/{label}"),
            size,
            size_is_estimate: false,
            measurable: true,
            method,
        }
    }

    fn plan(actions: Vec<PlannedAction>) -> CleanupPlan {
        CleanupPlan {
            plan_id: "p".into(),
            snapshot_id: "s".into(),
            total: actions.iter().map(|a| a.size).sum(),
            trash_total: 0,
            actions,
            permanent_available: false,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn confirm_label_names_the_total() {
        let p = plan(vec![
            action("a", 50_000_000_000, PlannedMethod::Delete),
            action("b", 2_500_000_000, PlannedMethod::Delete),
        ]);
        assert_eq!(confirm_label(&p), "Clean 52.5 GB");
    }

    #[test]
    fn groups_keep_plan_order_per_label() {
        let p = plan(vec![
            action("a", 1, PlannedMethod::Delete),
            action("b", 2, PlannedMethod::Delete),
            action("a", 3, PlannedMethod::Delete),
        ]);
        let got: Vec<(&str, usize)> = groups(&p).iter().map(|(l, a)| (*l, a.len())).collect();
        assert_eq!(got, [("a", 2), ("b", 1)]);
    }

    #[test]
    fn commands_read_with_their_program() {
        let m = PlannedMethod::Command {
            argv: vec![
                "/usr/bin/git".into(),
                "-C".into(),
                "/Users/x/app".into(),
                "worktree".into(),
                "remove".into(),
            ],
        };
        assert_eq!(method_words(&m), "Runs git");
    }

    const GB: u64 = 1_000_000_000;

    #[test]
    fn nothing_is_held_without_snapshots() {
        assert_eq!(held_by_snapshots(Some(0), Some(0), 20 * GB, 0), None);
    }

    #[test]
    fn nothing_is_held_when_free_space_grew_fully() {
        assert_eq!(held_by_snapshots(Some(0), Some(20 * GB), 20 * GB, 3), None);
    }

    #[test]
    fn the_shortfall_is_held_by_snapshots() {
        assert_eq!(
            held_by_snapshots(Some(10 * GB), Some(12 * GB), 20_200_000_000, 3),
            Some((18_200_000_000, 3))
        );
    }

    #[test]
    fn an_unknown_free_space_says_nothing() {
        assert_eq!(held_by_snapshots(None, Some(0), 20 * GB, 3), None);
    }

    #[test]
    fn the_held_line_names_size_and_count() {
        assert_eq!(
            held_line(18_200_000_000, 3),
            "18.2 GB is still held by 3 local Time Machine snapshots."
        );
    }

    #[test]
    fn contents_methods_say_the_folder_stays() {
        assert_eq!(
            method_words(&PlannedMethod::TrashContents),
            "Contents moved to Trash"
        );
    }
}
