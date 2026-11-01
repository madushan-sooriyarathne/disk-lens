//! Command palette (PRD 13.4.9): ⌘K over pages, recommendation rows, commands and
//! snapshot folders; `>` limits it to commands and `> filter …` sets a filter
//! (`filter.rs`). Also the two notice sheets it opens: a Pro feature's explanation
//! (until Phase 15 adds licensing) and Keyboard Shortcuts.
//!
//! Built on gpui-component's `Command` with its own filtering off: its match is a
//! substring test, so `groups` ranks with the fuzzy matcher the folder search uses.

use std::collections::HashSet;

use gpui::{
    Action, AnyElement, Context, Entity, FontWeight, IntoElement, Window, deferred, div,
    prelude::*, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::command::{Command, CommandGroup, CommandItem, CommandState};
use gpui_component::kbd::Kbd;
use gpui_component::{Sizable, Size};

use super::filter::Filter;
use super::format::format_bytes;
use super::inspector::display_path;
use super::model::{AppModel, Page};
use super::root;
use super::selection;
use super::theme::{self, Theme};
use crate::recommend::model::Recommendation;
use crate::rules::model::Rules;
use crate::view::{FolderHit, Fuzzy};

const WIDTH: f32 = 560.;
/// Folder results outside a `/` search, so pages and commands stay in view.
const FOLDERS_SHOWN: usize = 8;

/// The open palette.
pub struct Palette {
    pub state: Entity<CommandState>,
    /// `/` in Explore: folders under this path only.
    pub scope: Option<String>,
    pub query: String,
    /// The last folder search's results.
    pub folders: Vec<FolderHit>,
    /// What the list shows, rebuilt on every query and folder result.
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub label: &'static str,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Page(Page),
    Folder {
        path: String,
        label: String,
        size: u64,
    },
    /// Shows the row in the inspector on Recommended.
    Row {
        row_id: String,
        rule_id: String,
        label: String,
    },
    /// Stages or unstages one row's ready items (FR-KB-4, free).
    Stage {
        row_id: String,
        rule_id: String,
        label: String,
        staged: bool,
    },
    Command(Cmd),
    Filter(Filter),
    /// A filter that did not parse, shown disabled with the reason.
    Invalid(String),
    Pro(Pro),
}

/// App commands. None of them removes anything (S-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    Rescan,
    SelectSafe,
    ClearQueue,
    ExportScript,
    ToggleInspector,
    ShowQueue,
    HighlightCleanable,
    /// Puts `> filter ` in the search field.
    Filter,
    ClearFilter,
    Shortcuts,
}

/// Pro batch commands (FR-KB-4), listed with a Pro tag; they open the explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pro {
    StageRule,
    StageFilter,
    SavedFilters,
    SweepNow,
}

/// A sheet over the window that only explains something and closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Pro(Pro),
    Shortcuts,
}

const COMMANDS: [Cmd; 8] = [
    Cmd::Rescan,
    Cmd::SelectSafe,
    Cmd::ClearQueue,
    Cmd::ExportScript,
    Cmd::ToggleInspector,
    Cmd::ShowQueue,
    Cmd::HighlightCleanable,
    Cmd::Shortcuts,
];
const PRO: [Pro; 4] = [
    Pro::StageRule,
    Pro::StageFilter,
    Pro::SavedFilters,
    Pro::SweepNow,
];

/// Every shortcut, for Help › Keyboard Shortcuts (FR-KB-6).
pub const SHORTCUTS: [(&str, &[(&str, &str)]); 3] = [
    (
        "General",
        &[
            ("⌘K", "Command Palette"),
            ("⌘1", "Overview"),
            ("⌘2", "Explore"),
            ("⌘3", "Recommended"),
            ("⌘R", "Rescan"),
            ("⌘E", "Export Cleanup Script…"),
            ("⇧⌘R", "Reveal in Finder"),
            ("⌥⌘I", "Show Inspector"),
            ("⇧⌘H", "Highlight Cleanable"),
            ("⌃⌘S", "Show Sidebar"),
            ("⌘M", "Minimize"),
            ("Esc", "Close or go back"),
        ],
    ),
    (
        "Explore",
        &[
            ("h j k l, arrows", "Move between tiles"),
            ("Space", "Stage or unstage"),
            ("Return", "Open the folder"),
            ("Backspace, ⌘↑", "Up one level"),
            ("⌘Y", "Quick Look"),
            ("/", "Search folders here"),
        ],
    ),
    (
        "Recommended",
        &[
            ("j k, arrows", "Move between rows"),
            ("Space", "Stage or unstage"),
            ("Return", "Show in the inspector"),
        ],
    ),
];

impl Cmd {
    fn label(self) -> &'static str {
        match self {
            Self::Rescan => "Rescan",
            Self::SelectSafe => "Select Safe Items",
            Self::ClearQueue => "Clear Queue",
            Self::ExportScript => "Export Cleanup Script…",
            Self::ToggleInspector => "Show Inspector",
            Self::ShowQueue => "Show Queue",
            Self::HighlightCleanable => "Highlight Cleanable",
            Self::Filter => "Filter…",
            Self::ClearFilter => "Clear Filter",
            Self::Shortcuts => "Keyboard Shortcuts",
        }
    }

    fn action(self) -> Option<Box<dyn Action>> {
        Some(match self {
            Self::Rescan => Box::new(root::Rescan),
            Self::SelectSafe => Box::new(root::SelectSafe),
            Self::ClearQueue => Box::new(root::ClearQueue),
            Self::ExportScript => Box::new(root::ExportScript),
            Self::ToggleInspector => Box::new(root::ToggleInspector),
            Self::ShowQueue => Box::new(root::ShowQueue),
            Self::HighlightCleanable => Box::new(root::HighlightCleanable),
            Self::Filter | Self::ClearFilter | Self::Shortcuts => return None,
        })
    }
}

impl Pro {
    pub fn label(self) -> &'static str {
        match self {
            Self::StageRule => "Stage All Items of a Rule",
            Self::StageFilter => "Stage Everything Matching a Filter",
            Self::SavedFilters => "Saved Filters",
            Self::SweepNow => "Run Sweep Rule Now",
        }
    }

    fn explanation(self) -> &'static str {
        match self {
            Self::StageRule => {
                "Stage every ready item of one rule at once, such as all dormant Rust targets."
            }
            Self::StageFilter => {
                "Stage every ready item that matches a filter, such as safe items over 1 GB."
            }
            Self::SavedFilters => "Keep the filters you use often and apply them by name.",
            Self::SweepNow => "Run an auto-sweep rule now and stage what it finds.",
        }
    }
}

impl Entry {
    fn label(&self) -> String {
        match self {
            Self::Page(Page::Overview) => "Overview".into(),
            Self::Page(Page::Explore) => "Explore".into(),
            Self::Page(Page::Recommended) => "Recommended".into(),
            Self::Folder { label, .. } => format!("Go to {label}"),
            Self::Row { label, .. } => format!("Show {label}"),
            Self::Stage {
                label,
                staged: false,
                ..
            } => format!("Stage {label}"),
            Self::Stage { label, .. } => format!("Unstage {label}"),
            Self::Command(c) => c.label().into(),
            Self::Filter(f) => format!("Filter: {f}"),
            Self::Invalid(reason) => reason.clone(),
            Self::Pro(p) => p.label().into(),
        }
    }

    /// Rules also match on their ID ("node-modules-idle").
    fn rule_id(&self) -> Option<&str> {
        match self {
            Self::Row { rule_id, .. } | Self::Stage { rule_id, .. } => Some(rule_id),
            _ => None,
        }
    }

    fn action(&self) -> Option<Box<dyn Action>> {
        match self {
            Self::Page(Page::Overview) => Some(Box::new(root::ShowOverview)),
            Self::Page(Page::Explore) => Some(Box::new(root::ShowExplore)),
            Self::Page(Page::Recommended) => Some(Box::new(root::ShowRecommended)),
            Self::Command(c) => c.action(),
            _ => None,
        }
    }
}

/// The list for `query`. `scoped` is a `/` search: folders only. Folders arrive
/// ranked from the search; everything else is ranked here.
pub fn groups(
    query: &str,
    scoped: bool,
    recs: &[Recommendation],
    selected: &HashSet<String>,
    folders: &[FolderHit],
    filtering: bool,
    root_path: &str,
) -> Vec<Group> {
    let folder_entries = |n: usize| {
        folders
            .iter()
            .take(n)
            .map(|h| Entry::Folder {
                path: h.path.clone(),
                label: display_path(root_path, &h.path),
                size: h.size,
            })
            .collect()
    };
    if scoped {
        return keep(vec![group("Folders", folder_entries(usize::MAX))]);
    }
    let mut commands: Vec<Entry> = COMMANDS.into_iter().map(Entry::Command).collect();
    commands.push(Entry::Command(Cmd::Filter));
    if filtering {
        commands.push(Entry::Command(Cmd::ClearFilter));
    }
    let pro = PRO.into_iter().map(Entry::Pro).collect();
    if let Some(rest) = query.trim_start().strip_prefix('>') {
        let rest = rest.trim_start();
        if let Some(terms) = filter_terms(rest) {
            let entry = match Filter::parse(terms, Rules::embedded()) {
                Ok(f) => Entry::Filter(f),
                Err(e) => Entry::Invalid(e.to_string()),
            };
            return vec![group("Filter", vec![entry])];
        }
        return keep(vec![
            group("Commands", ranked(rest, commands)),
            group("Pro", ranked(rest, pro)),
        ]);
    }
    let pages = [Page::Overview, Page::Explore, Page::Recommended]
        .into_iter()
        .map(Entry::Page)
        .collect();
    let mut rows = Vec::new();
    for r in recs {
        rows.push(Entry::Row {
            row_id: r.row_id.clone(),
            rule_id: r.rule_id.clone(),
            label: r.label.clone(),
        });
        let ready = selection::ready_ids(r);
        if !ready.is_empty() {
            rows.push(Entry::Stage {
                row_id: r.row_id.clone(),
                rule_id: r.rule_id.clone(),
                label: r.label.clone(),
                staged: ready.iter().all(|id| selected.contains(*id)),
            });
        }
    }
    keep(vec![
        group("Pages", ranked(query, pages)),
        group("Recommendations", ranked(query, rows)),
        group("Commands", ranked(query, commands)),
        group("Pro", ranked(query, pro)),
        group("Folders", folder_entries(FOLDERS_SHOWN)),
    ])
}

/// `filter size>1GB` -> `size>1GB`; `filters` is not the filter command.
fn filter_terms(rest: &str) -> Option<&str> {
    let terms = rest.strip_prefix("filter")?;
    (terms.is_empty() || terms.starts_with(char::is_whitespace)).then_some(terms)
}

fn group(label: &'static str, entries: Vec<Entry>) -> Group {
    Group { label, entries }
}

/// Drops empty groups.
fn keep(groups: Vec<Group>) -> Vec<Group> {
    groups
        .into_iter()
        .filter(|g| !g.entries.is_empty())
        .collect()
}

/// Entries matching `query`, best first; ties keep their order.
fn ranked(query: &str, entries: Vec<Entry>) -> Vec<Entry> {
    let mut fuzzy = Fuzzy::new(query);
    let mut scored: Vec<(u32, Entry)> = entries
        .into_iter()
        .filter_map(|e| {
            let by_label = fuzzy.score(&e.label());
            let by_rule = e.rule_id().and_then(|id| fuzzy.score(id));
            by_label.max(by_rule).map(|s| (s, e))
        })
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(_, e)| e).collect()
}

/// The palette over the window, or `None` while closed.
pub fn render(
    model: &AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let palette = model.palette.as_ref()?;
    let t = theme::current(window);
    let entity = cx.entity();
    let mut command = Command::new(&palette.state)
        .filterable(false)
        .placeholder(if palette.scope.is_some() {
            "Search folders here"
        } else {
            "Search or type > for commands"
        })
        .max_h(px(380.))
        .empty(|_, _, _| div().p_3().child("No results"))
        .on_query({
            let entity = entity.clone();
            move |query, _, cx| {
                let query = query.to_owned();
                entity.update(cx, |m, cx| m.palette_query(query, cx));
            }
        })
        .on_confirm({
            let entity = entity.clone();
            move |ix, window, cx| entity.update(cx, |m, cx| m.palette_confirm(ix, window, cx))
        })
        .on_cancel(move |_, cx| entity.update(cx, |m, cx| m.close_palette(cx)));
    for g in &palette.groups {
        command = command.group(
            CommandGroup::new()
                .label(g.label)
                .items(g.entries.iter().map(|e| item(e, t))),
        );
    }
    Some(
        deferred(
            div()
                .id("palette-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .bg(t.scrim)
                .flex()
                .justify_center()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|m, _, _, cx| m.close_palette(cx)),
                )
                .child(
                    div()
                        .id("palette")
                        .mt(px(96.))
                        .w(px(WIDTH))
                        .h_auto()
                        .rounded(px(theme::R_WIDGET))
                        .bg(t.popover)
                        .shadow_lg()
                        .overflow_hidden()
                        .text_size(px(theme::T_BODY))
                        .text_color(t.label)
                        // Clicks inside stay inside.
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(command.bordered(false)),
                ),
        )
        .with_priority(2)
        .into_any_element(),
    )
}

/// One list row: label, a secondary detail, a Pro tag and the shortcut.
fn item(entry: &Entry, t: Theme) -> CommandItem {
    let label = entry.label();
    let detail = match entry {
        Entry::Folder { size, .. } => Some(format_bytes(*size)),
        _ => None,
    };
    let pro = matches!(entry, Entry::Pro(_));
    let action = entry.action();
    let item = CommandItem::new().label(label.clone());
    let item = if matches!(entry, Entry::Invalid(_)) {
        gpui_component::Disableable::disabled(item, true)
    } else {
        item
    };
    item.child(move |window, _| {
        let kbd = action
            .as_ref()
            .and_then(|a| Kbd::binding_for_action(a.as_ref(), Some(root::CONTEXT), window));
        div()
            .flex()
            .items_center()
            .gap_2()
            .w_full()
            .child(div().flex_1().min_w_0().truncate().child(label.clone()))
            .children(pro.then(|| {
                div()
                    .px(px(6.))
                    .rounded(px(4.))
                    .bg(t.selected)
                    .text_size(px(theme::T_SMALL))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.accent_text)
                    .child("Pro")
            }))
            .children(detail.clone().map(|d| {
                div()
                    .text_size(px(theme::T_SMALL))
                    .text_color(t.secondary)
                    .child(d)
            }))
            .children(kbd.map(|k| k.appearance(false)))
    })
}

/// The Pro or shortcuts sheet, or `None`.
pub fn notice(
    model: &AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let notice = model.notice?;
    let t = theme::current(window);
    let body = match notice {
        Notice::Pro(p) => div()
            .flex()
            .flex_col()
            .gap_2()
            .child(heading(p.label()))
            .child(p.explanation())
            .child(
                div()
                    .text_color(t.secondary)
                    .child("This is part of DiskLens Pro, which arrives in a later update."),
            ),
        Notice::Shortcuts => div()
            .flex()
            .flex_col()
            .gap_3()
            .child(heading("Keyboard Shortcuts"))
            .children(SHORTCUTS.iter().map(|(section, keys)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.secondary)
                            .child(*section),
                    )
                    .children(keys.iter().map(|(key, what)| {
                        div()
                            .flex()
                            .justify_between()
                            .child(*what)
                            .child(div().text_color(t.secondary).child(*key))
                    }))
            })),
    };
    Some(
        deferred(
            div()
                .id("notice-backdrop")
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
                        .id("notice")
                        .w(px(420.))
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
                        .child(
                            div().flex().justify_end().child(
                                Button::new("notice-close")
                                    .primary()
                                    .with_size(Size::Small)
                                    .label("Close")
                                    .on_click(cx.listener(|m, _, _, cx| m.close_notice(cx))),
                            ),
                        ),
                ),
        )
        .with_priority(3)
        .into_any_element(),
    )
}

fn heading(text: &'static str) -> impl IntoElement {
    div()
        .text_size(px(17.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recommend::model::{ItemStatus, MethodView, RecItem, Tier};

    fn rec(row_id: &str, label: &str, ready: bool) -> Recommendation {
        Recommendation {
            row_id: row_id.into(),
            rule_id: format!("{row_id}-rule"),
            label: label.into(),
            paths_hint: String::new(),
            tier: Tier::Safe,
            size: 1,
            size_is_estimate: false,
            items: vec![RecItem {
                item_id: format!("{row_id}:1"),
                path: "/i".into(),
                name: "i".into(),
                size: 1,
                status: if ready {
                    ItemStatus::Ready
                } else {
                    ItemStatus::Blocked {
                        reason: "busy".into(),
                    }
                },
                method: MethodView::Delete,
                idle_days: None,
            }],
        }
    }

    fn labels(groups: &[Group]) -> Vec<String> {
        groups
            .iter()
            .flat_map(|g| g.entries.iter().map(Entry::label))
            .collect()
    }

    fn list(query: &str) -> Vec<Group> {
        let recs = [
            rec("npm", "npm cache", true),
            rec("xcode", "Xcode DerivedData", false),
        ];
        let selected = HashSet::from(["npm:1".to_owned()]);
        let folders = [FolderHit {
            path: "/r/Projects".into(),
            name: "Projects".into(),
            size: 5,
        }];
        groups(query, false, &recs, &selected, &folders, false, "/r")
    }

    #[test]
    fn the_angle_bracket_lists_commands_only() {
        let names: Vec<&str> = list(">").iter().map(|g| g.label).collect();
        assert_eq!(names, ["Commands", "Pro"]);
    }

    #[test]
    fn a_staged_row_offers_unstage() {
        assert!(labels(&list("npm")).contains(&"Unstage npm cache".to_owned()));
    }

    #[test]
    fn a_blocked_row_offers_no_stage() {
        let rows: Vec<Group> = list("xcode")
            .into_iter()
            .filter(|g| g.label == "Recommendations")
            .collect();
        assert_eq!(labels(&rows), ["Show Xcode DerivedData"]);
    }

    #[test]
    fn rows_match_on_their_rule_id() {
        assert!(labels(&list("xcode-rule")).contains(&"Show Xcode DerivedData".to_owned()));
    }

    #[test]
    fn folders_show_under_the_root_as_tilde() {
        assert!(labels(&list("")).contains(&"Go to ~/Projects".to_owned()));
    }

    #[test]
    fn a_filter_parses_into_one_entry() {
        assert!(matches!(
            list("> filter tier:safe")[0].entries[..],
            [Entry::Filter(_)]
        ));
    }

    #[test]
    fn a_bad_filter_shows_the_reason() {
        assert_eq!(
            labels(&list("> filter tier:all")),
            ["Unknown tier: tier:all (use safe or review)"]
        );
    }

    #[test]
    fn filters_is_not_the_filter_command() {
        assert_ne!(list("> filters")[0].label, "Filter");
    }

    #[test]
    fn clear_filter_shows_only_while_filtering() {
        let g = groups(">clear fil", false, &[], &HashSet::new(), &[], true, "/r");
        assert_eq!(labels(&g)[0], "Clear Filter");
    }

    #[test]
    fn a_scoped_search_lists_folders_only() {
        let folders = [FolderHit {
            path: "/r/a".into(),
            name: "a".into(),
            size: 1,
        }];
        let g = groups(
            "a",
            true,
            &[rec("npm", "a", true)],
            &HashSet::new(),
            &folders,
            false,
            "/r",
        );
        assert_eq!(labels(&g), ["Go to ~/a"]);
    }

    #[test]
    fn pro_entries_carry_the_tag() {
        let pro = list(">")
            .into_iter()
            .find(|g| g.label == "Pro")
            .map(|g| g.entries.len());
        assert_eq!(pro, Some(4));
    }
}
