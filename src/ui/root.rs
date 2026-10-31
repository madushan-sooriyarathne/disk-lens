//! Window layout by phase (PRD section 9) and window-wide keys.

use gpui::{
    App, Context, KeyBinding, Menu, MenuItem, SystemMenuType, Window, actions, div, prelude::*, px,
};

use super::model::{AppModel, Cleanup, Page, Phase};
use super::{cleanup, screens, shell, theme};

actions!(
    disklens,
    [
        Back,
        FocusNext,
        FocusPrev,
        Quit,
        Rescan,
        ExportScript,
        RevealInFinder,
        AddToQueue,
        SelectSafe,
        ClearQueue,
        CommandPalette,
        ShowOverview,
        ShowExplore,
        ShowRecommended,
        ToggleInspector,
        ShowQueue,
        HighlightCleanable,
        QuickLook,
        ToggleSidebar,
        Minimize,
        Zoom,
        KeyboardShortcuts,
    ]
);

const CONTEXT: &str = "DiskLens";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Back, Some(CONTEXT)),
        KeyBinding::new("tab", FocusNext, Some(CONTEXT)),
        KeyBinding::new("shift-tab", FocusPrev, Some(CONTEXT)),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-r", Rescan, Some(CONTEXT)),
        KeyBinding::new("cmd-e", ExportScript, Some(CONTEXT)),
        KeyBinding::new("cmd-shift-r", RevealInFinder, Some(CONTEXT)),
        KeyBinding::new("cmd-k", CommandPalette, Some(CONTEXT)),
        KeyBinding::new("cmd-1", ShowOverview, Some(CONTEXT)),
        KeyBinding::new("cmd-2", ShowExplore, Some(CONTEXT)),
        KeyBinding::new("cmd-3", ShowRecommended, Some(CONTEXT)),
        KeyBinding::new("alt-cmd-i", ToggleInspector, Some(CONTEXT)),
        KeyBinding::new("cmd-shift-h", HighlightCleanable, Some(CONTEXT)),
        KeyBinding::new("cmd-y", QuickLook, Some(CONTEXT)),
        KeyBinding::new("ctrl-cmd-s", ToggleSidebar, Some(CONTEXT)),
        KeyBinding::new("cmd-m", Minimize, Some(CONTEXT)),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}

/// The menu bar in PRD FR-WIN-4. Items whose feature arrives in a later phase
/// are shown disabled, so the layout and shortcuts are learnable now.
pub fn set_menus(cx: &mut App) {
    let later = |name: &'static str, action: Box<dyn gpui::Action>| MenuItem::Action {
        name: name.into(),
        action,
        os_action: None,
        checked: false,
        disabled: true,
    };
    cx.set_menus([
        Menu::new("DiskLens").items([
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Quit DiskLens", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Rescan", Rescan),
            // The save panel arrives with the queue (Phase 11).
            later("Export Cleanup Script…", Box::new(ExportScript)),
            MenuItem::action("Reveal in Finder", RevealInFinder),
        ]),
        Menu::new("Edit").items([
            MenuItem::action("Add to Queue", AddToQueue),
            MenuItem::action("Select Safe Items", SelectSafe),
            MenuItem::action("Clear Queue", ClearQueue),
        ]),
        Menu::new("View").items([
            later("Command Palette", Box::new(CommandPalette)),
            MenuItem::separator(),
            MenuItem::action("Overview", ShowOverview),
            MenuItem::action("Explore", ShowExplore),
            MenuItem::action("Recommended", ShowRecommended),
            MenuItem::separator(),
            MenuItem::action("Show Inspector", ToggleInspector),
            later("Show Queue", Box::new(ShowQueue)),
            MenuItem::action("Highlight Cleanable", HighlightCleanable),
            later("Quick Look", Box::new(QuickLook)),
            MenuItem::separator(),
            MenuItem::action("Show Sidebar", ToggleSidebar),
        ]),
        Menu::new("Window").items([
            MenuItem::action("Minimize", Minimize),
            MenuItem::action("Zoom", Zoom),
        ]),
        Menu::new("Help").items([later("Keyboard Shortcuts", Box::new(KeyboardShortcuts))]),
    ]);
}

impl Render for AppModel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme::current(window);
        let body = div()
            .relative()
            .size_full()
            .flex()
            .text_color(t.label)
            .font_family(theme::SANS)
            .font_features(theme::numeric());
        let full_window = |body: gpui::Div| body.p_4().bg(t.content);
        let body = match self.phase {
            Phase::Loading => full_window(body),
            Phase::Onboarding => full_window(body).child(screens::onboarding(self, window, cx)),
            Phase::Idle => full_window(body).child(screens::idle(self, window, cx)),
            Phase::Scanning => full_window(body).child(screens::scanning(self, window, cx)),
            Phase::Error => full_window(body).child(screens::error(self, window, cx)),
            // The sidebar stays translucent over the blurred window; the rest is opaque.
            Phase::Ready => body
                .when(self.sidebar_visible, |b| {
                    b.child(shell::sidebar(self, t, cx))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .bg(t.content)
                        .child(shell::toolbar(self, t, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .gap(px(theme::GAP_CONTENT))
                                        .px(px(theme::GAP_CONTENT))
                                        .pb(px(theme::GAP_CONTENT))
                                        .pt_1()
                                        .children(screens::update_banner(self, window, cx))
                                        .child(shell::page(self, window, cx)),
                                )
                                .when(self.inspector_visible, |d| d.child(shell::inspector(t))),
                        ),
                ),
        };
        let dialog = cleanup::render(self, window, cx);
        let toast = cleanup::toast(self, window);
        body.children(dialog)
            .children(toast)
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrev, window, cx| window.focus_prev(cx))
            .on_action(|_: &Minimize, window, _| window.minimize_window())
            .on_action(|_: &Zoom, window, _| window.zoom_window())
            .on_action(cx.listener(|this, _: &Rescan, _, cx| {
                if this.scan.is_none() {
                    this.start_scan(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &RevealInFinder, _, cx| {
                if this.phase == Phase::Ready {
                    this.reveal(this.path.clone(), cx);
                }
            }))
            .on_action(cx.listener(|this, _: &AddToQueue, _, cx| {
                if let Some(row) = this.focused_row.clone() {
                    this.add_row(&row, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SelectSafe, _, cx| this.select_safe(cx)))
            .on_action(cx.listener(|this, _: &ClearQueue, _, cx| this.clear_selection(cx)))
            .on_action(
                cx.listener(|this, _: &ShowOverview, _, cx| this.set_page(Page::Overview, cx)),
            )
            .on_action(cx.listener(|this, _: &ShowExplore, _, cx| this.set_page(Page::Explore, cx)))
            .on_action(
                cx.listener(|this, _: &ShowRecommended, _, cx| {
                    this.set_page(Page::Recommended, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &ToggleInspector, _, cx| this.toggle_inspector(cx)))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| this.toggle_sidebar(cx)))
            .on_action(cx.listener(|this, _: &HighlightCleanable, _, cx| this.toggle_highlight(cx)))
            // Esc closes the cleanup dialog (not while it runs), then the context
            // menu, then the detail drawer, otherwise goes up one folder.
            .on_action(cx.listener(|this, _: &Back, _, cx| {
                if let Some(stage) = &this.cleanup {
                    if !matches!(stage, Cleanup::Running { .. }) {
                        this.close_cleanup(cx);
                    }
                } else if this.explorer.close_menu() {
                    cx.notify();
                } else if this.focused_row.is_some() {
                    this.focus_row(None, cx);
                } else {
                    this.up(cx);
                }
            }))
    }
}
