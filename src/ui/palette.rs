//! Command palette (PRD 13.4.9): ⌘K over pages, recommendation rows, commands and
//! snapshot folders; `>` limits it to commands and `> filter …` sets a filter
//! (`filter.rs`). Also the two notice sheets it opens: the Pro sheet and Keyboard Shortcuts.
//!
//! Drawn in the look of Raycast's compact window: a large field, rows with an icon and
//! a kind, and a footer naming what Return does. The field is gpui-component's `Input`;
//! the list is drawn here, because the kit's `Command` fixes a small field and plain
//! rows. `groups` ranks with the fuzzy matcher the folder search uses.

use std::collections::HashSet;

use gpui::{
    Action, AnyElement, BoxShadow, Context, Entity, FontWeight, IntoElement, Keystroke,
    ScrollHandle, SharedString, Subscription, Window, deferred, div, point, prelude::*, px,
    relative, svg,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::kbd::Kbd;
use gpui_component::{Sizable, Size};

use super::filter::Filter;
use super::format::format_bytes;
use super::inspector::display_path;
use super::keymap;
use super::model::{AppModel, Page};
use super::root;
use super::selection;
use super::settings_window::site_url;
use super::theme::{self, Theme};
use crate::recommend::model::Recommendation;
use crate::rules::model::Rules;
use crate::view::{FolderHit, Fuzzy};

const WIDTH: f32 = 640.;
const BAR_H: f32 = 52.;
const ROW_H: f32 = 40.;
const ROW_TEXT: f32 = 14.;
/// About eight and a half rows, so a cut-off row shows there is more.
const LIST_MAX_H: f32 = 360.;
/// Folder results outside a `/` search, so pages and commands stay in view.
const FOLDERS_SHOWN: usize = 8;
/// Entries under an empty field: Raycast's compact window shows a few, not everything.
const SUGGESTIONS: usize = 4;

/// The open palette.
pub struct Palette {
    pub input: Entity<InputState>,
    /// The highlighted entry, a flat index over all groups.
    pub selected: usize,
    pub scroll: ScrollHandle,
    pub _query: Subscription,
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
    /// Pro: stages every ready item of one row (FR-KB-4).
    StageRule {
        row_id: String,
        rule_id: String,
        label: String,
    },
    /// Pro: stages the ready items of every row `filter` matches; `count` is how many.
    StageMatching {
        filter: Filter,
        count: usize,
    },
    /// Pro: a saved filter, by its text.
    ApplySaved(String),
    ForgetSaved(String),
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
    /// Pro: keeps the active filter.
    SaveFilter,
    Shortcuts,
}

/// What the Pro sheet explains. The first three are the batch commands (FR-KB-4), listed
/// with a Pro tag until a license makes them real; `About` is the sheet behind the
/// sidebar link and "More with Pro".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pro {
    StageRule,
    StageFilter,
    SavedFilters,
    SweepNow,
    About,
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
            Self::SaveFilter => "Save Filter",
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
            Self::Filter | Self::ClearFilter | Self::SaveFilter | Self::Shortcuts => return None,
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
            Self::About => "DiskLens Pro",
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
            Self::About => {
                "Pro adds scanners for more kinds of clutter, batch staging from the command palette and saved filters."
            }
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
            Self::StageRule { label, .. } => format!("Stage All {label}"),
            Self::StageMatching { filter, count, .. } => {
                let noun = if *count == 1 { "item" } else { "items" };
                format!("Stage {count} {noun} matching {filter}")
            }
            Self::ApplySaved(text) => format!("Apply Saved Filter: {text}"),
            Self::ForgetSaved(text) => format!("Forget Saved Filter: {text}"),
            Self::Pro(p) => p.label().into(),
        }
    }

    /// Rules also match on their ID ("node-modules-idle").
    fn rule_id(&self) -> Option<&str> {
        match self {
            Self::Row { rule_id, .. }
            | Self::Stage { rule_id, .. }
            | Self::StageRule { rule_id, .. } => Some(rule_id),
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

/// What the list is built from.
pub struct Inputs<'a> {
    pub recs: &'a [Recommendation],
    pub selected: &'a HashSet<String>,
    pub folders: &'a [FolderHit],
    /// The active filter, if any.
    pub filter: Option<&'a Filter>,
    pub root_path: &'a str,
    /// With a license the Pro entries run; without one they explain.
    pub pro: bool,
    pub saved_filters: &'a [String],
}

/// The list for `query`. `scoped` is a `/` search: folders only. Folders arrive
/// ranked from the search; everything else is ranked here.
pub fn groups(query: &str, scoped: bool, inputs: &Inputs) -> Vec<Group> {
    let Inputs {
        recs,
        selected,
        folders,
        filter,
        root_path,
        pro,
        ..
    } = *inputs;
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
    let empty = query.trim().is_empty();
    if scoped {
        let n = if empty { SUGGESTIONS } else { usize::MAX };
        return keep(vec![group("Folders", folder_entries(n))]);
    }
    if empty {
        return vec![group(
            "Suggestions",
            vec![
                Entry::Page(Page::Overview),
                Entry::Page(Page::Explore),
                Entry::Page(Page::Recommended),
                Entry::Command(Cmd::Rescan),
            ],
        )];
    }
    let mut commands: Vec<Entry> = COMMANDS.into_iter().map(Entry::Command).collect();
    commands.push(Entry::Command(Cmd::Filter));
    if let Some(f) = filter {
        commands.push(Entry::Command(Cmd::ClearFilter));
        if pro && !inputs.saved_filters.contains(&f.to_string()) {
            commands.push(Entry::Command(Cmd::SaveFilter));
        }
    }
    if let Some(rest) = query.trim_start().strip_prefix('>') {
        let rest = rest.trim_start();
        // `> stage tier:safe` stages what a filter matches. Words that are no filter, such
        // as "dormant rust targets", search the rules below instead.
        if pro
            && let Some(Ok(f)) =
                terms_after(rest, "stage").map(|t| Filter::parse(t, Rules::embedded()))
        {
            let count = matching_ids(recs, &f).len();
            let entry = if count == 0 {
                Entry::Invalid(format!("No ready items match {f}"))
            } else {
                Entry::StageMatching { filter: f, count }
            };
            return vec![group("Stage", vec![entry])];
        }
        if let Some(terms) = terms_after(rest, "filter") {
            let entry = match Filter::parse(terms, Rules::embedded()) {
                Ok(f) => Entry::Filter(f),
                Err(e) => Entry::Invalid(e.to_string()),
            };
            return vec![group("Filter", vec![entry])];
        }
        return keep(vec![
            group("Commands", ranked(rest, commands)),
            group("Pro", ranked(rest, pro_entries(inputs, true))),
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
        group("Pro", ranked(query, pro_entries(inputs, false))),
        group("Folders", folder_entries(FOLDERS_SHOWN)),
    ])
}

/// The entry at flat index `ix` across all groups.
pub fn entry_at(groups: &[Group], ix: usize) -> Option<&Entry> {
    groups.iter().flat_map(|g| &g.entries).nth(ix)
}

/// The list's child index of entry `ix`: every group adds a heading before its rows.
pub fn child_index(groups: &[Group], ix: usize) -> usize {
    let mut before = 0;
    for (headings, g) in groups.iter().enumerate() {
        if ix < before + g.entries.len() {
            return ix + headings + 1;
        }
        before += g.entries.len();
    }
    ix + groups.len()
}

/// The ready items of the rows `filter` matches, never a blocked one (S-11).
pub fn matching_ids(recs: &[Recommendation], filter: &Filter) -> Vec<String> {
    let rules = Rules::embedded();
    recs.iter()
        .filter(|r| {
            filter.matches_row(
                r,
                rules.rule(&r.rule_id).map_or("", |x| x.category.as_str()),
            )
        })
        .flat_map(selection::ready_ids)
        .map(String::from)
        .collect()
}

/// Pro entries. Without a license they are the placeholders that open the sheet. With one
/// they are the real commands; `with_rules` adds one "Stage All" per rule, which only the
/// `>` list shows, since a typed query already finds the free Stage entries.
fn pro_entries(inputs: &Inputs, with_rules: bool) -> Vec<Entry> {
    if !inputs.pro {
        return PRO.into_iter().map(Entry::Pro).collect();
    }
    let mut entries = Vec::new();
    if with_rules {
        for r in inputs.recs {
            if selection::ready_ids(r)
                .iter()
                .any(|id| !inputs.selected.contains(*id))
            {
                entries.push(Entry::StageRule {
                    row_id: r.row_id.clone(),
                    rule_id: r.rule_id.clone(),
                    label: r.label.clone(),
                });
            }
        }
    }
    for text in inputs.saved_filters {
        entries.push(Entry::ApplySaved(text.clone()));
        entries.push(Entry::ForgetSaved(text.clone()));
    }
    entries.push(Entry::Pro(Pro::SweepNow));
    entries
}

/// `filter size>1GB` -> `size>1GB`; `filters` is not the filter command.
fn terms_after<'a>(rest: &'a str, word: &str) -> Option<&'a str> {
    let terms = rest.strip_prefix(word)?;
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
            // With the label, so "stage dormant rust targets" finds "Stage All …".
            let by_rule = e
                .rule_id()
                .and_then(|id| fuzzy.score(&format!("{} {}", e.label(), rule_text(id))));
            by_label.max(by_rule).map(|s| (s, e))
        })
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(_, e)| e).collect()
}

/// A rule's ID and its `keywords` as one text, so "dormant rust targets" finds
/// `rust-target-idle` (FR-KB-2). IDs outside `rules.toml` match on the ID alone.
fn rule_text(id: &str) -> String {
    match Rules::embedded().rule(id) {
        Some(rule) => format!("{id} {}", rule.keywords.join(" ")),
        None => id.to_owned(),
    }
}

/// The palette over the window, or `None` while closed.
pub fn render(
    model: &AppModel,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let palette = model.palette.as_ref()?;
    let t = theme::current(window);
    let typed = !palette.query.trim().is_empty();
    let mut list = Vec::new();
    let mut ix = 0;
    for g in &palette.groups {
        list.push(
            div()
                .px_2()
                .pt_2()
                .pb_1()
                .text_size(px(theme::T_SMALL))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.secondary)
                .child(g.label)
                .into_any_element(),
        );
        for e in &g.entries {
            list.push(row(e, ix, ix == palette.selected, t, window, cx));
            ix += 1;
        }
    }
    let scope = palette
        .scope
        .as_ref()
        .map(|s| display_path(&model.root_path, s));
    let bar = div()
        .flex()
        .items_center()
        .gap_3()
        .h(px(BAR_H))
        .px_4()
        .flex_none()
        .child(
            svg()
                .path("icons/ui/search.svg")
                .size(px(18.))
                .flex_none()
                .text_color(t.tertiary),
        )
        .child(
            div().flex_1().min_w_0().child(
                Input::new(&palette.input)
                    .appearance(false)
                    .with_size(Size::Size(px(20.))),
            ),
        )
        .children(scope.map(|s| {
            div()
                .flex_none()
                .text_size(px(theme::T_SMALL))
                .text_color(t.secondary)
                .child(format!("In {s}"))
        }));
    let body = if !list.is_empty() {
        let footer = entry_at(&palette.groups, palette.selected).and_then(verb);
        div()
            .child(div().h(px(1.)).bg(t.separator))
            .child(
                div()
                    .id("palette-list")
                    .track_scroll(&palette.scroll)
                    .overflow_y_scroll()
                    .max_h(px(LIST_MAX_H))
                    .p_2()
                    .children(list),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .h(px(ROW_H))
                    .px_3()
                    .border_t_1()
                    .border_color(t.separator)
                    .children(footer.map(|v| {
                        div()
                            .text_size(px(theme::T_SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(v)
                    }))
                    .children(
                        footer
                            .and_then(|_| Keystroke::parse("enter").ok())
                            .map(Kbd::new),
                    ),
            )
    } else if typed {
        div().child(div().h(px(1.)).bg(t.separator)).child(
            div()
                .py_4()
                .text_center()
                .text_color(t.secondary)
                .child("No results"),
        )
    } else {
        div()
    };
    Some(
        deferred(
            // No scrim: the palette floats like Raycast's window. The backdrop only
            // catches the click outside.
            div()
                .id("palette-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .flex()
                .justify_center()
                // Without it the row flex stretches the panel to the window's height.
                .items_start()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|m, _, _, cx| m.close_palette(cx)),
                )
                .child(
                    div()
                        .id("palette")
                        .key_context(keymap::PALETTE)
                        .on_action(
                            cx.listener(|m, _: &keymap::SelectUp, _, cx| m.palette_move(-1, cx)),
                        )
                        .on_action(
                            cx.listener(|m, _: &keymap::SelectDown, _, cx| m.palette_move(1, cx)),
                        )
                        .on_action(cx.listener(|m, _: &keymap::OpenSelected, window, cx| {
                            m.palette_confirm(window, cx)
                        }))
                        .mt(relative(0.14))
                        .w(px(WIDTH))
                        .max_w(relative(0.9))
                        .flex()
                        .flex_col()
                        .rounded(px(theme::R_WIDGET))
                        .border_1()
                        .border_color(t.separator)
                        .bg(t.popover)
                        .shadow(vec![
                            BoxShadow {
                                color: gpui::black().opacity(0.35),
                                offset: point(px(0.), px(24.)),
                                blur_radius: px(64.),
                                spread_radius: px(0.),
                                inset: false,
                            },
                            BoxShadow {
                                color: gpui::black().opacity(0.2),
                                offset: point(px(0.), px(2.)),
                                blur_radius: px(8.),
                                spread_radius: px(0.),
                                inset: false,
                            },
                        ])
                        .overflow_hidden()
                        .text_size(px(theme::T_BODY))
                        .text_color(t.label)
                        // Clicks inside stay inside.
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(bar)
                        .child(body),
                ),
        )
        .with_priority(2)
        .into_any_element(),
    )
}

/// One list row: icon, title, then a Pro tag, the folder size, the shortcut or the kind.
fn row(
    entry: &Entry,
    ix: usize,
    selected: bool,
    t: Theme,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let invalid = matches!(entry, Entry::Invalid(_));
    let trailing = match entry {
        Entry::Pro(_) => div()
            .px(px(6.))
            .rounded(px(4.))
            .bg(t.selected)
            .text_size(px(theme::T_SMALL))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t.accent_text)
            .child("Pro")
            .into_any_element(),
        Entry::Folder { size, .. } => div()
            .text_color(t.secondary)
            .child(format_bytes(*size))
            .into_any_element(),
        _ => match entry
            .action()
            .and_then(|a| Kbd::binding_for_action(a.as_ref(), Some(root::CONTEXT), window))
        {
            Some(k) => k.appearance(false).into_any_element(),
            None => div()
                .text_color(t.secondary)
                .child(kind(entry))
                .into_any_element(),
        },
    };
    div()
        .id(("palette-row", ix))
        .flex()
        .items_center()
        .gap(px(10.))
        .h(px(ROW_H))
        .px_2()
        .rounded(px(theme::R_CONTROL))
        .when(selected, |d| d.bg(t.selected))
        .when(!invalid, |d| {
            // Mouse move, not hover: arrow keys scroll rows under a still pointer.
            d.on_mouse_move(cx.listener(move |m, _, _, cx| m.palette_select(ix, cx)))
                .on_click(cx.listener(move |m, _, window, cx| {
                    m.palette_select(ix, cx);
                    m.palette_confirm(window, cx);
                }))
        })
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(24.))
                .child(
                    svg()
                        .path(SharedString::from(format!("icons/ui/{}.svg", icon(entry))))
                        .size(px(16.))
                        .text_color(t.secondary),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(ROW_TEXT))
                .text_color(if invalid { t.secondary } else { t.label })
                .child(entry.label()),
        )
        .child(div().flex_none().child(trailing))
        .into_any_element()
}

/// The line icon (`assets/icons/ui/`) at the start of a row.
fn icon(entry: &Entry) -> &'static str {
    match entry {
        Entry::Page(Page::Overview) => "grid",
        Entry::Page(Page::Explore) => "map",
        Entry::Page(Page::Recommended) | Entry::Row { .. } => "sparkle",
        Entry::Folder { .. } => "folder",
        Entry::Stage { .. } | Entry::StageRule { .. } | Entry::StageMatching { .. } => "tray",
        Entry::ApplySaved(_) => "search",
        Entry::ForgetSaved(_) => "x",
        Entry::Command(c) => match c {
            Cmd::Rescan => "rescan",
            Cmd::SelectSafe | Cmd::ShowQueue | Cmd::ClearQueue => "tray",
            Cmd::ExportScript | Cmd::Shortcuts => "doc",
            Cmd::ToggleInspector => "inspector",
            Cmd::HighlightCleanable => "grid",
            Cmd::Filter | Cmd::ClearFilter | Cmd::SaveFilter => "search",
        },
        Entry::Filter(_) => "search",
        Entry::Pro(_) => "lock",
        Entry::Invalid(_) => "x",
    }
}

/// What a row is, shown on the right when it has no shortcut.
fn kind(entry: &Entry) -> &'static str {
    match entry {
        Entry::Page(_) => "Page",
        Entry::Folder { .. } => "Folder",
        Entry::Row { .. } => "Recommendation",
        Entry::Stage { .. }
        | Entry::StageRule { .. }
        | Entry::StageMatching { .. }
        | Entry::ForgetSaved(_) => "Action",
        Entry::ApplySaved(_) => "Filter",
        Entry::Command(_) => "Command",
        Entry::Filter(_) | Entry::Invalid(_) => "Filter",
        Entry::Pro(_) => "Pro",
    }
}

/// What Return does to the highlighted row, for the footer.
fn verb(entry: &Entry) -> Option<&'static str> {
    Some(match entry {
        Entry::Page(_) => "Open Page",
        Entry::Folder { .. } => "Go to Folder",
        Entry::Row { .. } => "Show",
        Entry::Stage { staged: false, .. } => "Stage",
        Entry::Stage { .. } => "Unstage",
        Entry::StageRule { .. } | Entry::StageMatching { .. } => "Stage",
        Entry::ApplySaved(_) => "Apply Filter",
        Entry::ForgetSaved(_) => "Forget",
        Entry::Command(_) => "Run",
        Entry::Filter(_) => "Apply Filter",
        Entry::Pro(_) => "Learn More",
        Entry::Invalid(_) => return None,
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
    // The sheet offers a purchase only to someone who has no Pro yet.
    let offer = matches!(notice, Notice::Pro(_)) && !model.license.is_pro();
    let body = match notice {
        Notice::Pro(p) => div()
            .flex()
            .flex_col()
            .gap_2()
            .child(heading(p.label()))
            .child(p.explanation())
            .children((p == Pro::SweepNow).then(|| {
                div()
                    .text_color(t.secondary)
                    .child("Auto-sweep rules arrive in a later update.")
            }))
            .children(offer.then(|| {
                div()
                    .text_color(t.secondary)
                    .child("Everything DiskLens finds still cleans for free.")
            })),
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
                        .w(px(if offer { 540. } else { 420. }))
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
                            div()
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap_2()
                                .child({
                                    let close = Button::new("notice-close")
                                        .with_size(Size::Small)
                                        .label("Close")
                                        .on_click(cx.listener(|m, _, _, cx| m.close_notice(cx)));
                                    // Close is the default only when nothing is offered.
                                    if offer { close } else { close.primary() }
                                })
                                .children(offer.then(|| {
                                    div()
                                        .flex()
                                        .gap_2()
                                        .child(
                                            Button::new("pro-license")
                                                .with_size(Size::Small)
                                                .label("Enter License…")
                                                .on_click(cx.listener(|m, _, _, cx| {
                                                    m.close_notice(cx);
                                                    m.open_settings(None, cx);
                                                })),
                                        )
                                        .child(
                                            Button::new("pro-trial")
                                                .with_size(Size::Small)
                                                .label("Start 7-Day Trial")
                                                .on_click(cx.listener(|_, _, _, cx| {
                                                    cx.open_url(&site_url("trial"))
                                                })),
                                        )
                                        .child(
                                            Button::new("pro-buy")
                                                .primary()
                                                .with_size(Size::Small)
                                                .label("Buy DiskLens Pro")
                                                .on_click(cx.listener(|_, _, _, cx| {
                                                    cx.open_url(&site_url("buy"))
                                                })),
                                        )
                                })),
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
                rehydrate: None,
                active_work: None,
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
        groups(
            query,
            false,
            &Inputs {
                recs: &recs,
                selected: &selected,
                folders: &folders,
                ..inputs(&[], &HashSet::new())
            },
        )
    }

    fn inputs<'a>(recs: &'a [Recommendation], selected: &'a HashSet<String>) -> Inputs<'a> {
        Inputs {
            recs,
            selected,
            folders: &[],
            filter: None,
            root_path: "/r",
            pro: false,
            saved_filters: &[],
        }
    }

    fn pro_list(query: &str, recs: &[Recommendation], filter: Option<&Filter>) -> Vec<Group> {
        let selected = HashSet::new();
        groups(
            query,
            false,
            &Inputs {
                pro: true,
                filter,
                ..inputs(recs, &selected)
            },
        )
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
    fn rows_match_on_their_rule_keywords() {
        let recs = [Recommendation {
            rule_id: "rust-target-idle".into(),
            ..rec("rust", "Rust build output in idle projects", false)
        }];
        let groups = groups(
            "dormant rust targets",
            false,
            &inputs(&recs, &HashSet::new()),
        );
        assert_eq!(labels(&groups), ["Show Rust build output in idle projects"]);
    }

    #[test]
    fn folders_show_under_the_root_as_tilde() {
        assert!(labels(&list("proj")).contains(&"Go to ~/Projects".to_owned()));
    }

    #[test]
    fn an_empty_query_suggests_the_pages_and_rescan() {
        assert_eq!(
            labels(&list(" ")),
            ["Overview", "Explore", "Recommended", "Rescan"]
        );
    }

    #[test]
    fn a_scoped_empty_query_shows_four_folders() {
        let folders: Vec<FolderHit> = (0..6)
            .map(|i| FolderHit {
                path: format!("/r/{i}"),
                name: i.to_string(),
                size: 1,
            })
            .collect();
        let g = groups(
            "",
            true,
            &Inputs {
                folders: &folders,
                ..inputs(&[], &HashSet::new())
            },
        );
        assert_eq!(g[0].entries.len(), 4);
    }

    #[test]
    fn child_index_skips_group_headings() {
        // The second entry of group 2 has both headings before it.
        let g = list(">");
        assert_eq!(
            child_index(&g, g[0].entries.len() + 1),
            g[0].entries.len() + 3
        );
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
        let filter = Filter::parse("tier:safe", Rules::embedded()).unwrap();
        let g = pro_list(">clear fil", &[], Some(&filter));
        assert_eq!(labels(&g)[0], "Clear Filter");
    }

    #[test]
    fn a_scoped_search_lists_folders_only() {
        let folders = [FolderHit {
            path: "/r/a".into(),
            name: "a".into(),
            size: 1,
        }];
        let recs = [rec("npm", "a", true)];
        let g = groups(
            "a",
            true,
            &Inputs {
                folders: &folders,
                ..inputs(&recs, &HashSet::new())
            },
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

    fn rust_row() -> Recommendation {
        Recommendation {
            rule_id: "rust-target-idle".into(),
            ..rec("rust", "Rust build output in idle projects", true)
        }
    }

    #[test]
    fn pro_stage_finds_a_rule_by_its_keywords() {
        let g = pro_list("> stage dormant rust targets", &[rust_row()], None);
        assert_eq!(labels(&g), ["Stage All Rust build output in idle projects"]);
    }

    #[test]
    fn pro_stage_all_is_not_offered_once_the_rule_is_staged() {
        let recs = [rust_row()];
        let selected = HashSet::from(["rust:1".to_owned()]);
        let g = groups(
            "> stage dormant rust targets",
            false,
            &Inputs {
                pro: true,
                ..inputs(&recs, &selected)
            },
        );
        assert!(g.is_empty());
    }

    #[test]
    fn pro_stage_with_a_filter_counts_only_ready_items() {
        let recs = [rec("npm", "npm cache", true), rec("xcode", "Xcode", false)];
        let g = pro_list("> stage tier:safe", &recs, None);
        assert_eq!(labels(&g), ["Stage 1 item matching tier:safe"]);
    }

    #[test]
    fn pro_stage_with_a_filter_that_matches_nothing_says_so() {
        let g = pro_list("> stage tier:review", &[rec("npm", "npm", true)], None);
        assert_eq!(labels(&g), ["No ready items match tier:review"]);
    }

    #[test]
    fn free_stage_with_a_filter_offers_nothing_to_run() {
        let recs = [rec("npm", "npm cache", true)];
        let g = groups("> stage tier:safe", false, &inputs(&recs, &HashSet::new()));
        assert!(!labels(&g).iter().any(|l| l.starts_with("Stage 1")));
    }

    #[test]
    fn matching_ids_leave_out_blocked_items() {
        let recs = [rec("xcode", "Xcode", false)];
        let filter = Filter::parse("tier:safe", Rules::embedded()).unwrap();
        assert!(matching_ids(&recs, &filter).is_empty());
    }

    #[test]
    fn pro_offers_to_save_the_active_filter() {
        let filter = Filter::parse("tier:safe", Rules::embedded()).unwrap();
        let g = pro_list("> save", &[], Some(&filter));
        assert_eq!(labels(&g), ["Save Filter"]);
    }

    #[test]
    fn pro_does_not_offer_to_save_a_filter_twice() {
        let filter = Filter::parse("tier:safe", Rules::embedded()).unwrap();
        let saved = ["tier:safe".to_owned()];
        let selected = HashSet::new();
        let g = groups(
            "> save",
            false,
            &Inputs {
                pro: true,
                filter: Some(&filter),
                saved_filters: &saved,
                ..inputs(&[], &selected)
            },
        );
        assert!(!labels(&g).contains(&"Save Filter".to_owned()));
    }

    #[test]
    fn pro_lists_a_saved_filter_to_apply_and_forget() {
        let saved = ["size>1GB".to_owned()];
        let selected = HashSet::new();
        let g = groups(
            "> saved filter",
            false,
            &Inputs {
                pro: true,
                saved_filters: &saved,
                ..inputs(&[], &selected)
            },
        );
        assert_eq!(
            labels(&g),
            [
                "Apply Saved Filter: size>1GB",
                "Forget Saved Filter: size>1GB"
            ]
        );
    }
}
