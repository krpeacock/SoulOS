use crate::form::{A11yHints, Action, Component, ComponentType, Form, Interaction, Rect, Trigger};
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

fn make_button(id: &str, x: i32, y: i32, interactions: Vec<Interaction>) -> Component {
    Component {
        id: id.into(),
        class: "primary".into(),
        type_: ComponentType::Button,
        bounds: Rect { x, y, w: 60, h: 20 },
        properties: BTreeMap::from([("label".into(), id.into())]),
        a11y: A11yHints {
            label: id.into(),
            role: "button".into(),
            hint: None,
        },
        interactions,
        binding: None,
    }
}

#[test]
fn test_query_selector() {
    let mut form = Form::new("test");
    form.components.push(make_button("btn1", 0, 0, Vec::new()));

    assert!(form.query_selector("#btn1").is_some());
    assert!(form.query_selector(".primary").is_some());
    assert!(form.query_selector("[btn1]").is_some());
    assert!(form.query_selector("#nonexistent").is_none());
}

#[test]
fn test_dispatch_returns_action_for_matching_trigger() {
    let mut form = Form::new("test");
    form.components.push(make_button(
        "btn_save",
        10,
        280,
        vec![Interaction {
            trigger: Trigger::OnTap,
            action: Action::SaveRecord(0),
        }],
    ));

    let action = form.dispatch(Trigger::OnTap, "btn_save");
    assert!(matches!(action, Some(Action::SaveRecord(0))));
}

#[test]
fn test_dispatch_returns_none_for_wrong_trigger() {
    let mut form = Form::new("test");
    form.components.push(make_button(
        "btn_save",
        10,
        280,
        vec![Interaction {
            trigger: Trigger::OnTap,
            action: Action::SaveRecord(0),
        }],
    ));

    assert!(form.dispatch(Trigger::OnChange, "btn_save").is_none());
}

#[test]
fn test_dispatch_returns_none_for_unknown_component() {
    let form = Form::new("test");
    assert!(form.dispatch(Trigger::OnTap, "ghost").is_none());
}

#[test]
fn test_tap_dispatch_hits_component_with_action() {
    let mut form = Form::new("test");
    form.components.push(make_button(
        "btn_nav",
        10,
        10,
        vec![Interaction {
            trigger: Trigger::OnTap,
            action: Action::Navigate("screen2".into()),
        }],
    ));

    let result = form.tap_dispatch(40, 20);
    assert!(result.is_some());
    let (comp, action) = result.unwrap();
    assert_eq!(comp.id, "btn_nav");
    assert!(matches!(action, Some(Action::Navigate(_))));
}

#[test]
fn test_tap_dispatch_hits_component_with_no_action() {
    let mut form = Form::new("test");
    form.components.push(make_button("lbl", 10, 10, Vec::new()));

    let result = form.tap_dispatch(40, 20);
    assert!(result.is_some());
    let (comp, action) = result.unwrap();
    assert_eq!(comp.id, "lbl");
    assert!(action.is_none());
}

#[test]
fn test_tap_dispatch_misses_returns_none() {
    let mut form = Form::new("test");
    form.components.push(make_button("btn", 10, 10, Vec::new()));

    assert!(form.tap_dispatch(200, 200).is_none());
}

#[test]
fn test_dispatch_first_matching_trigger_wins() {
    let mut form = Form::new("test");
    form.components.push(make_button(
        "btn",
        0,
        0,
        vec![
            Interaction {
                trigger: Trigger::OnTap,
                action: Action::CloseApp,
            },
            Interaction {
                trigger: Trigger::OnTap,
                action: Action::DeleteRecord,
            },
        ],
    ));

    assert!(matches!(
        form.dispatch(Trigger::OnTap, "btn"),
        Some(Action::CloseApp)
    ));
}

// --- MenuSheet tests --------------------------------------------------------

#[cfg(test)]
mod menu_tests {
    use crate::menu::{MenuItem, MenuSheet};
    use soul_core::{Event, HardButton, KeyCode};

    // Layout geometry (must match menu.rs constants):
    //   TITLE_BAR_H = 15, SHEET_BORDER = 1, SHEET_PAD = 2, ITEM_INSET = 3
    //   ITEM_H = 22, ITEM_SLOT = 23
    //   Item 0 y-range: [18, 40)
    //   Item 1 y-range: [41, 63)
    //   Item 2 y-range: [64, 86)
    //   Item x-range: [4, 236)
    //   Sheet for 3 items: y [15, 89)

    const ITEMS: &[MenuItem<'static>] = &[
        MenuItem::new("Cut"),
        MenuItem::with_shortcut("Copy", 'C'),
        MenuItem::with_shortcut("Paste", 'V'),
    ];

    const ITEMS_WITH_DISABLED: &[MenuItem<'static>] = &[
        MenuItem::new("Save"),
        MenuItem::disabled("Undo"),
        MenuItem::new("Delete"),
    ];

    fn pen_down(x: i16, y: i16) -> Event { Event::PenDown { x, y } }
    fn pen_up(x: i16, y: i16) -> Event { Event::PenUp { x, y } }
    fn pen_move(x: i16, y: i16) -> Event { Event::PenMove { x, y } }

    #[test]
    fn starts_closed() {
        let menu = MenuSheet::new();
        assert!(!menu.is_open());
    }

    #[test]
    fn opens_on_menu_event() {
        let mut menu = MenuSheet::new();
        let out = menu.handle(&Event::Menu, ITEMS);
        assert!(menu.is_open());
        assert!(out.committed.is_none());
        assert!(out.dirty.is_some());
    }

    #[test]
    fn closes_on_second_menu_event() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        let out = menu.handle(&Event::Menu, ITEMS);
        assert!(!menu.is_open());
        assert!(out.committed.is_none());
        assert!(out.dirty.is_some());
    }

    #[test]
    fn closes_on_appstop() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        let out = menu.handle(&Event::AppStop, ITEMS);
        assert!(!menu.is_open());
        assert!(out.committed.is_none());
    }

    #[test]
    fn no_dirty_when_closed_and_not_menu_event() {
        let mut menu = MenuSheet::new();
        let out = menu.handle(&pen_down(100, 30), ITEMS);
        assert!(!menu.is_open());
        assert!(out.committed.is_none());
        assert!(out.dirty.is_none());
    }

    #[test]
    fn commits_on_pen_tap_item_0() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        menu.handle(&pen_down(120, 25), ITEMS); // item 0 y-range [18, 40)
        let out = menu.handle(&pen_up(120, 25), ITEMS);
        assert_eq!(out.committed, Some(0));
        assert!(!menu.is_open());
        assert!(out.dirty.is_some());
    }

    #[test]
    fn commits_on_pen_tap_item_1() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        menu.handle(&pen_down(120, 50), ITEMS); // item 1 y-range [41, 63)
        let out = menu.handle(&pen_up(120, 50), ITEMS);
        assert_eq!(out.committed, Some(1));
        assert!(!menu.is_open());
    }

    #[test]
    fn drag_to_item_and_lift_commits_that_item() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        menu.handle(&pen_down(120, 25), ITEMS); // start on item 0
        menu.handle(&pen_move(120, 50), ITEMS); // drag to item 1
        let out = menu.handle(&pen_up(120, 50), ITEMS); // lift on item 1
        assert_eq!(out.committed, Some(1));
        assert!(!menu.is_open());
    }

    #[test]
    fn pen_up_outside_sheet_closes_no_commit() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        menu.handle(&pen_down(120, 25), ITEMS);
        let out = menu.handle(&pen_up(120, 200), ITEMS); // below 3-item sheet (y < 89 would be inside)
        assert!(out.committed.is_none());
        assert!(!menu.is_open());
    }

    #[test]
    fn disabled_item_not_committed_on_tap() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS_WITH_DISABLED);
        menu.handle(&pen_down(120, 41), ITEMS_WITH_DISABLED); // item 1 = "Undo" (disabled)
        let out = menu.handle(&pen_up(120, 41), ITEMS_WITH_DISABLED);
        assert!(out.committed.is_none());
        assert!(menu.is_open()); // stays open; disabled items don't commit
    }

    #[test]
    fn shortcut_key_commits_matching_item() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        let out = menu.handle(&Event::Key(KeyCode::Char('C')), ITEMS); // "Copy" shortcut
        assert_eq!(out.committed, Some(1));
        assert!(!menu.is_open());
    }

    #[test]
    fn shortcut_key_case_insensitive() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        let out = menu.handle(&Event::Key(KeyCode::Char('v')), ITEMS); // 'v' matches shortcut 'V'
        assert_eq!(out.committed, Some(2));
    }

    #[test]
    fn shortcut_key_no_match_stays_open() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        let out = menu.handle(&Event::Key(KeyCode::Char('X')), ITEMS);
        assert!(out.committed.is_none());
        assert!(menu.is_open());
    }

    #[test]
    fn arrow_down_moves_selection() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        // Initial selected = 0; ArrowDown moves to 1.
        menu.handle(&Event::Key(KeyCode::ArrowDown), ITEMS);
        // Enter commits the selection.
        let out = menu.handle(&Event::Key(KeyCode::Enter), ITEMS);
        assert_eq!(out.committed, Some(1));
    }

    #[test]
    fn arrow_up_from_first_stays_at_first() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        menu.handle(&Event::Key(KeyCode::ArrowUp), ITEMS);
        let out = menu.handle(&Event::Key(KeyCode::Enter), ITEMS);
        assert_eq!(out.committed, Some(0)); // can't go above 0
    }

    #[test]
    fn arrow_down_skips_disabled() {
        // [enabled, disabled, enabled] — ArrowDown from 0 should skip 1 → land on 2.
        let items: &[MenuItem<'static>] = &[
            MenuItem::new("Save"),
            MenuItem::disabled("Undo"),
            MenuItem::new("Delete"),
        ];
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, items);
        menu.handle(&Event::Key(KeyCode::ArrowDown), items);
        let out = menu.handle(&Event::Key(KeyCode::Enter), items);
        assert_eq!(out.committed, Some(2));
    }

    #[test]
    fn page_down_equivalent_to_arrow_down() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        menu.handle(&Event::ButtonDown(HardButton::PageDown), ITEMS);
        let out = menu.handle(&Event::Key(KeyCode::Enter), ITEMS);
        assert_eq!(out.committed, Some(1));
    }

    #[test]
    fn app_a_commits_current_selection() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        menu.handle(&Event::Key(KeyCode::ArrowDown), ITEMS);
        let out = menu.handle(&Event::ButtonDown(HardButton::AppA), ITEMS);
        assert_eq!(out.committed, Some(1));
    }

    #[test]
    fn absorbs_tick_when_open() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        let out = menu.handle(&Event::Tick(1000), ITEMS);
        assert!(out.committed.is_none());
        assert!(out.dirty.is_none());
        assert!(menu.is_open()); // still open
    }

    #[test]
    fn dirty_rect_on_open_covers_sheet() {
        let mut menu = MenuSheet::new();
        let out = menu.handle(&Event::Menu, ITEMS);
        let r = out.dirty.expect("should be dirty on open");
        // Sheet top must be at TITLE_BAR_H = 15
        assert_eq!(r.top_left.y, 15);
        // Sheet must be full-width
        assert_eq!(r.size.width, 240);
        // Height must accommodate 3 items
        assert!(r.size.height > 0);
    }

    #[test]
    fn dirty_rect_on_close_covers_sheet() {
        let mut menu = MenuSheet::new();
        menu.handle(&Event::Menu, ITEMS);
        let out = menu.handle(&Event::Menu, ITEMS);
        let r = out.dirty.expect("should be dirty on close");
        assert_eq!(r.top_left.y, 15);
        assert_eq!(r.size.width, 240);
    }
}
