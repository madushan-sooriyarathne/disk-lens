//! Cleanup dialog over the window: confirm, progress, results (PRD section
//! 7.6), plus the toast. The confirm button only reacts to a click; no key
//! binding triggers it.

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, Hsla, Window, deferred, div, prelude::*, px,
    relative, rgb,
};

use super::format::{format_bytes, format_size};
use super::model::{AppModel, Cleanup};
use super::recommendations::{checkbox, tier_pill, tilde};
use super::selection::RowState;
use super::theme::{self, MONO, Theme};
use crate::cleanup::model::{CleanupPlan, CleanupSummary, PlannedAction, PlannedMethod};

const WIDTH: f32 = 520.;
/// Tailwind `red-600`, as on the error screen.
const ERROR_TEXT: u32 = 0xdc2626;

/// The dialog for the model's cleanup stage, if one is open.
pub fn render(
    model: &AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let t = theme::current(window);
    let root = model.meta.as_ref().map_or("", |m| m.root.as_str());
    let dry_run = model.source.is_dry_run();
    let card = match model.cleanup.as_ref()? {
        Cleanup::Planning => card(&t).child(muted(&t, "Preparing the cleanup…")),
        Cleanup::Confirm { plan, permanent } => confirm(&t, plan, *permanent, root, dry_run, cx),
        Cleanup::Running {
            plan,
            done,
            current,
        } => running(&t, plan, *done, current.as_deref()),
        Cleanup::Results { summary } => results(&t, summary, cx),
    };
    Some(
        deferred(
            div()
                .id("cleanup-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(Hsla { a: 0.35, ..t.text })
                .occlude()
                .child(card),
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
                        .bg(t.text)
                        .text_color(t.card)
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

/// The method in plain words, paths shortened under `root`.
fn method_words(m: &PlannedMethod, root: &str) -> String {
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
            let args: Vec<String> = argv.iter().skip(1).map(|a| tilde(a, root)).collect();
            format!(
                "Runs `{}`",
                [vec![program.to_owned()], args].concat().join(" ")
            )
        }
    }
}

fn card(t: &Theme) -> Div {
    div()
        .w(px(WIDTH))
        .max_h(relative(0.85))
        .flex()
        .flex_col()
        .p_6()
        .rounded(px(12.))
        .border_1()
        .border_color(t.card_border)
        .bg(t.card)
        .text_color(t.text)
}

fn heading(text: String) -> Div {
    div()
        .text_size(px(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

fn muted(t: &Theme, text: impl Into<String>) -> Div {
    div()
        .mt_2()
        .text_size(px(13.))
        .line_height(px(20.))
        .text_color(t.muted)
        .child(text.into())
}

fn button(t: &Theme, id: &'static str, label: String, primary: bool) -> gpui::Stateful<Div> {
    let b = div()
        .id(id)
        .px_4()
        .py(px(6.))
        .rounded_lg()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .child(label);
    if primary {
        b.bg(t.text).text_color(t.card)
    } else {
        b.border_1().border_color(t.card_border)
    }
}

fn confirm(
    t: &Theme,
    plan: &CleanupPlan,
    permanent: bool,
    root: &str,
    dry_run: bool,
    cx: &mut Context<AppModel>,
) -> Div {
    let count = plan.actions.len();
    let mut list = div()
        .id("cleanup-actions")
        .mt_4()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .border_t_1()
        .border_color(t.divider);
    for (label, actions) in groups(plan) {
        let mut group = div().py_2().border_b_1().border_color(t.divider).child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .child(tier_pill(t, actions[0].tier))
                .child(label.to_owned()),
        );
        for a in actions {
            group = group.child(
                div()
                    .mt_1()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_size(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(div().truncate().child(tilde(&a.path, root)))
                            .child(
                                div()
                                    .text_color(t.muted)
                                    .child(method_words(&a.method, root)),
                            ),
                    )
                    .child(
                        div()
                            .font_family(MONO)
                            .child(format_size(a.size, a.size_is_estimate)),
                    ),
            );
        }
        list = list.child(group);
    }
    let mut c = card(t)
        .child(heading(format!(
            "Clean up {count} {}",
            if count == 1 { "item" } else { "items" }
        )))
        .child(list);
    for w in &plan.warnings {
        c = c.child(muted(t, format!("Left out: {w}")));
    }
    if plan.permanent_available {
        c = c.child(
            div()
                .id("cleanup-permanent")
                .mt_3()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(13.))
                .cursor_pointer()
                .child(checkbox(
                    t,
                    "cleanup-permanent-box",
                    if permanent {
                        RowState::All
                    } else {
                        RowState::None
                    },
                    false,
                    16.,
                ))
                .child("Delete permanently instead of moving to Trash")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.set_permanent(!permanent, cx);
                })),
        );
    }
    if plan.trash_total > 0 {
        c = c.child(muted(
            t,
            format!(
                "{} goes to the Trash. That space is freed only when you empty the Trash.",
                format_bytes(plan.trash_total)
            ),
        ));
    }
    if dry_run {
        c = c.child(muted(
            t,
            "Dry run: every step is checked and logged, nothing is removed.",
        ));
    }
    let mut clean = button(t, "cleanup-confirm", confirm_label(plan), true);
    clean = if count == 0 {
        clean.opacity(0.4)
    } else {
        clean.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.confirm_cleanup(cx)))
    };
    c.child(
        div()
            .mt_5()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                button(t, "cleanup-cancel", "Cancel".into(), false)
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
    card(t)
        .child(heading(format!(
            "Cleaning… {done} of {}",
            plan.actions.len()
        )))
        .child(muted(t, name))
        .child(
            div()
                .mt_4()
                .h(px(6.))
                .w_full()
                .rounded_full()
                .bg(t.divider)
                .child(
                    div()
                        .h_full()
                        .rounded_full()
                        .bg(t.text)
                        .w(relative(done as f32 / total as f32)),
                ),
        )
}

fn results(t: &Theme, s: &CleanupSummary, cx: &mut Context<AppModel>) -> Div {
    let mut c = card(t).child(heading(if s.dry_run {
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
            .mt_2()
            .flex()
            .justify_between()
            .text_size(px(13.))
            .child(label.to_owned())
            .child(div().font_family(MONO).child(format_bytes(value)))
    };
    c = c
        .child(line(freed, s.freed))
        .child(line(trashed, s.trashed));
    if !s.skipped.is_empty() {
        c = c.child(muted(t, format!("Skipped {}:", s.skipped.len())));
        for item in &s.skipped {
            c = c.child(
                div()
                    .mt_1()
                    .text_size(px(12.))
                    .text_color(rgb(ERROR_TEXT))
                    .child(format!("{}: {}", item.name, item.reason)),
            );
        }
    }
    c.child(
        div().mt_5().flex().justify_end().child(
            button(t, "cleanup-done", "Done".into(), true)
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
    fn commands_read_with_short_program_and_paths() {
        let m = PlannedMethod::Command {
            argv: vec![
                "/usr/bin/git".into(),
                "-C".into(),
                "/Users/x/app".into(),
                "worktree".into(),
                "remove".into(),
                "/Users/x/app/.claude/worktrees/w1".into(),
            ],
        };
        assert_eq!(
            method_words(&m, "/Users/x"),
            "Runs `git -C ~/app worktree remove ~/app/.claude/worktrees/w1`"
        );
    }

    #[test]
    fn contents_methods_say_the_folder_stays() {
        assert_eq!(
            method_words(&PlannedMethod::TrashContents, "/Users/x"),
            "Contents moved to Trash"
        );
    }
}
