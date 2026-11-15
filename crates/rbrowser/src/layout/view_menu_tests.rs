use super::*;
use crate::layout::test_support::{config, setup};

#[test]
fn the_add_menu_lists_the_hidden_breakpoints_by_width_then_add_all_and_new_size() {
    let config = config();
    let items = add_items(&config, &["desktop".to_owned(), "mobile".to_owned()], &[]);
    let actions: Vec<Option<AddAction>> = items.iter().map(|item| item.action.clone()).collect();
    assert_eq!(
        actions,
        [
            Some(AddAction::Add("mobile".to_owned())),
            Some(AddAction::Add("desktop".to_owned())),
            Some(AddAction::AddAll),
            Some(AddAction::NewSize),
        ]
    );
    assert_eq!(items[0].label, "mobile  400 \u{d7} 800");
    assert!(items.iter().all(AddMenuItem::enabled));
}

#[test]
fn with_one_hidden_breakpoint_there_is_no_add_all_and_with_none_a_disabled_note() {
    let config = config();
    let items = add_items(&config, &["tablet".to_owned()], &[]);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].action, Some(AddAction::Add("tablet".to_owned())));
    assert_eq!(items[1].action, Some(AddAction::NewSize));
    let items = add_items(&config, &[], &[]);
    assert_eq!(items.len(), 2);
    assert!(!items[0].enabled());
    // "All breakpoints are open" once `locales/parts/views-add-remove.json` is merged.
    assert_eq!(items[0].label, rb_i18n::t(KEY_ALL_OPEN));
    assert_eq!(items[1].action, Some(AddAction::NewSize));
}

#[test]
fn an_empty_cell_names_its_breakpoint() {
    let mut config = config();
    config.breakpoints[1].label = Some("Tablet".to_owned());
    // "hidden: Tablet" once `locales/parts/views-add-remove.json` is merged.
    let expected = |label: &str| rb_i18n::t_with(KEY_HIDDEN_CELL, &[("label", label)]);
    assert_eq!(hidden_cell_label(&config, "tablet"), expected("Tablet"));
    assert_eq!(hidden_cell_label(&config, "mobile"), expected("mobile"));
}

#[tokio::test]
async fn the_view_menu_offers_remove_activate_size_pop_out_and_on_the_canvas_center() {
    let setup = setup(config());
    let host = setup.core.host();
    let views = host.views();
    let first = views[0].id;
    let second = views[1].id;
    host.set_active_view(first).expect("activates");
    let actions = |view, canvas| -> Vec<ViewAction> {
        view_items(host, view, canvas)
            .iter()
            .map(|item| item.action)
            .collect()
    };
    assert_eq!(
        actions(first, false),
        [ViewAction::Remove, ViewAction::Size, ViewAction::PopOut]
    );
    assert_eq!(
        actions(second, true),
        [
            ViewAction::Remove,
            ViewAction::Activate,
            ViewAction::Size,
            ViewAction::PopOut,
            ViewAction::Center
        ]
    );
    let labels: Vec<String> = view_items(host, second, true)
        .into_iter()
        .map(|item| item.label)
        .collect();
    // "Remove view", "Set as active", "Size…", "Pop out into its own window", "Center on this
    // view" once `locales/parts/views-add-remove.json` is merged.
    let expected: Vec<String> = [
        "views.menu.remove",
        "views.menu.activate",
        "views.menu.size",
        "views.menu.pop_out",
        "views.menu.center",
    ]
    .into_iter()
    .map(rb_i18n::t)
    .collect();
    assert_eq!(labels, expected);
    host.pop_out(second, crate::host::PopOutOptions::default())
        .expect("pops out");
    assert_eq!(actions(second, false)[3], ViewAction::PopIn);
    // Activate runs on the host at once.
    run_view_action(&setup.core, second, ViewAction::Activate);
    assert_eq!(host.active_view(), Some(second));
}

// RB-080-46: the free canvas's menu is native now; its rows are the add menu's and the canvas's.
#[test]
fn the_free_canvas_adds_zoom_to_fit_and_center_here() {
    let config = config();
    let items = area_items(&config, &["tablet".to_owned()], &[]);
    let actions: Vec<Option<AddAction>> = items.iter().map(|item| item.action.clone()).collect();
    assert_eq!(
        actions,
        [
            Some(AddAction::Add("tablet".to_owned())),
            Some(AddAction::NewSize),
            Some(AddAction::ZoomToFit),
            Some(AddAction::CenterHere),
        ]
    );
    assert!(items.iter().all(|item| !item.label.is_empty()));
}

// RB-080-46: the native strips heard no right-click at all (a CEF button reports its press
// without the mouse button); the window's own listener asks which strip a right-click hit.
#[test]
fn a_right_click_hits_the_strip_whose_rectangle_holds_it() {
    use crate::page_window::ScreenRect;
    use rb_engine::ViewId;
    let rect = |x, y, width, height| ScreenRect {
        x,
        y,
        width,
        height,
    };
    let strips = [
        (Strip::View(ViewId(1)), rect(0, 44, 400, 20)),
        (Strip::View(ViewId(2)), rect(408, 44, 400, 20)),
        (Strip::Hidden, rect(816, 44, 400, 20)),
        // A strip that is not laid out hits nothing.
        (Strip::View(ViewId(9)), rect(0, 0, 0, 0)),
    ];
    assert_eq!(strip_at(0, 44, &strips), Some(Strip::View(ViewId(1))));
    assert_eq!(strip_at(399, 63, &strips), Some(Strip::View(ViewId(1))));
    assert_eq!(strip_at(500, 50, &strips), Some(Strip::View(ViewId(2))));
    assert_eq!(strip_at(900, 50, &strips), Some(Strip::Hidden));
    // The gap between two strips, the page below a strip, the toolbar above.
    assert_eq!(strip_at(404, 50, &strips), None);
    assert_eq!(strip_at(100, 64, &strips), None);
    assert_eq!(strip_at(100, 43, &strips), None);
    assert_eq!(strip_at(0, 0, &strips), None);
    assert_eq!(strip_at(i32::MAX, i32::MAX, &strips), None);
}

// RB-080-60: the view menu switches the footer under every view too - a check row after the
// view's own rows that runs `ui.view_footer.toggle`.
#[tokio::test]
async fn the_view_menu_ends_in_the_footer_switch_checked_while_it_shows() {
    let setup = setup(config());
    let core = &setup.core;
    let view = core.host().views()[0].id;
    let rows = menu_items(core, view, false);
    let own = view_items(core.host(), view, false);
    assert_eq!(rows[..own.len()], own[..], "the view's own rows first");
    assert!(own.iter().all(|item| item.checked.is_none()));
    let footer = rows.last().expect("rows");
    assert_eq!(footer.action, ViewAction::Footer);
    assert_eq!(footer.checked, Some(true), "the footer is on by default");
    assert_eq!(footer.label, rb_i18n::t(KEY_FOOTER));
    let mut user = core.user_settings();
    user.ui.view_footer.visible = false;
    core.replace_user_settings(user);
    let hidden = menu_items(core, view, true);
    assert_eq!(hidden.last().map(|item| item.checked), Some(Some(false)));
    assert_eq!(hidden[hidden.len() - 2].action, ViewAction::Center);
}

#[tokio::test]
async fn the_footer_row_runs_the_toggle() {
    let setup = setup(config());
    let core = &setup.core;
    let view = core.host().views()[0].id;
    assert!(core.user_settings().ui.view_footer.visible);
    run_view_action(core, view, ViewAction::Footer);
    let mut switched = false;
    for _ in 0..200 {
        if !core.user_settings().ui.view_footer.visible {
            switched = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(switched, "ui.viewFooter.visible was switched off");
}

// RB-080-60 review: the switch for every view stands apart, after the view's engine submenu.
#[test]
fn the_native_menus_draw_the_switch_for_every_view_last_and_set_apart() {
    let row = |action, checked| ViewMenuItem {
        action,
        label: String::new(),
        checked,
    };
    let items = [
        row(ViewAction::Remove, None),
        row(ViewAction::PopOut, None),
        row(ViewAction::Footer, Some(true)),
    ];
    assert_eq!(
        layout_rows(&items, true),
        [
            MenuRow::Item(0),
            MenuRow::Item(1),
            MenuRow::Engines,
            MenuRow::Separator,
            MenuRow::Item(2),
        ]
    );
    assert_eq!(
        layout_rows(&items, false),
        [
            MenuRow::Item(0),
            MenuRow::Item(1),
            MenuRow::Separator,
            MenuRow::Item(2),
        ]
    );
    assert_eq!(
        layout_rows(&items[..2], true).last(),
        Some(&MenuRow::Engines)
    );
}

// RB-080-60 review: the strip's size list carries the same view rows as the right-click menu -
// the footer switch included - without its own "Size..."; none with `ui.viewContextMenu` off.
#[tokio::test]
async fn the_size_list_has_the_view_menus_rows_and_the_footer_switch() {
    let setup = setup(config());
    let core = &setup.core;
    let view = core.host().views()[0].id;
    let rows = size_list_items(core, view);
    let expected: Vec<ViewMenuItem> = menu_items(core, view, false)
        .into_iter()
        .filter(|item| item.action != ViewAction::Size)
        .collect();
    assert_eq!(rows, expected);
    assert_eq!(
        rows.last().map(|item| item.action),
        Some(ViewAction::Footer)
    );
    let mut user = core.user_settings();
    user.ui.view_context_menu = false;
    core.replace_user_settings(user);
    assert!(size_list_items(core, view).is_empty());
}
