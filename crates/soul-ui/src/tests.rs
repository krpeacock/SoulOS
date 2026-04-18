use crate::form::{Action, Form, Component, ComponentType, Interaction, Rect, A11yHints, Trigger};
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
        a11y: A11yHints { label: id.into(), role: "button".into() },
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
    form.components.push(make_button("btn_save", 10, 280, vec![
        Interaction { trigger: Trigger::OnTap, action: Action::SaveRecord(0) },
    ]));

    let action = form.dispatch(Trigger::OnTap, "btn_save");
    assert!(matches!(action, Some(Action::SaveRecord(0))));
}

#[test]
fn test_dispatch_returns_none_for_wrong_trigger() {
    let mut form = Form::new("test");
    form.components.push(make_button("btn_save", 10, 280, vec![
        Interaction { trigger: Trigger::OnTap, action: Action::SaveRecord(0) },
    ]));

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
    form.components.push(make_button("btn_nav", 10, 10, vec![
        Interaction { trigger: Trigger::OnTap, action: Action::Navigate("screen2".into()) },
    ]));

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
    form.components.push(make_button("btn", 0, 0, vec![
        Interaction { trigger: Trigger::OnTap, action: Action::CloseApp },
        Interaction { trigger: Trigger::OnTap, action: Action::DeleteRecord },
    ]));

    assert!(matches!(form.dispatch(Trigger::OnTap, "btn"), Some(Action::CloseApp)));
}
