use crate::form::{Form, Component, ComponentType, Rect, A11yHints};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

#[test]
fn test_query_selector() {
    let mut form = Form::new("test");
    form.components.push(Component {
        id: "btn1".into(),
        class: "primary".into(),
        type_: ComponentType::Button,
        bounds: Rect { x: 0, y: 0, w: 10, h: 10 },
        properties: BTreeMap::from([("label".into(), "Click Me".into())]),
        a11y: A11yHints { label: "btn1".into(), role: "button".into() },
        interactions: Vec::new(),
        binding: None,
    });

    assert!(form.query_selector("#btn1").is_some());
    assert!(form.query_selector(".primary").is_some());
    assert!(form.query_selector("[Click Me]").is_some());
    assert!(form.query_selector("#nonexistent").is_none());
}
