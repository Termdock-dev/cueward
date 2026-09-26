use super::tests::Fixture;
use super::*;

fn query(
    condition: WaitCondition,
    role: &str,
    name: Option<&str>,
    identifier: Option<&str>,
) -> WaitOptions {
    WaitOptions {
        condition,
        selector: Some(WaitSelector {
            role: role.into(),
            name: name.map(str::to_owned),
            identifier: identifier.map(str::to_owned),
        }),
        value: None,
        timeout_ms: 4000,
        interval_ms: 100,
    }
}

fn press_named(fixture: &Fixture, name: &str) {
    let tree = fixture.inspect();
    let node = tree
        .accessibility
        .nodes
        .iter()
        .find(|n| n.name == name)
        .expect("named control");
    press(node.target.as_deref().expect("action target")).expect("press fixture control");
}

fn check_async_conditions(fixture: &Fixture, token: &str) {
    let tree = fixture.inspect();
    let progress = tree
        .accessibility
        .nodes
        .iter()
        .find(|n| n.identifier.as_deref() == Some("progress"))
        .expect("progress identifier exposed");
    let mut value = query(
        WaitCondition::ValueEquals,
        "AXTextField",
        None,
        Some("status"),
    );
    value.value = Some("ready".into());
    value.timeout_ms = 200;
    let timeout = wait_for_window(token, &value).expect("bounded wait");
    assert_eq!(timeout.status, WaitStatus::TimedOut);
    press_named(fixture, "Start");
    value.timeout_ms = 4000;
    let matched = wait_for_window(token, &value).expect("wait for delayed value");
    assert_eq!(matched.status, WaitStatus::Matched);
    assert!(matched.matched_ref.is_some());
    let enabled = query(WaitCondition::Enabled, "AXButton", Some("Continue"), None);
    assert_eq!(
        wait_for_window(token, &enabled).expect("enabled").status,
        WaitStatus::Matched
    );
    let absent = query(
        WaitCondition::ElementAbsent,
        &progress.role,
        None,
        Some("progress"),
    );
    assert_eq!(
        wait_for_window(token, &absent)
            .expect("progress disappeared")
            .status,
        WaitStatus::Matched
    );
    let ambiguous = query(WaitCondition::Enabled, "AXButton", None, None);
    assert_eq!(
        wait_for_window(token, &ambiguous)
            .expect("ambiguous")
            .status,
        WaitStatus::Ambiguous
    );
}

#[test]
#[ignore = "requires an unlocked desktop and Accessibility/Screen Recording permissions"]
fn wait_observes_async_conditions_timeout_ambiguity_and_window_lifecycle() {
    let fixture =
        Fixture::launch_source(include_str!("wait_fixture.swift"), "Cueward Wait Fixture");
    let image = fixture.directory.path().join("window.png");
    let shot = snapshot_window(fixture.window_id, false, image.to_str()).expect("snapshot");
    check_async_conditions(&fixture, &shot.input_target);
    press_named(&fixture, "Rename");
    let exists = query(
        WaitCondition::ElementExists,
        "AXButton",
        Some("Continue"),
        None,
    );
    assert_eq!(
        wait_for_window(&shot.input_target, &exists)
            .expect("old identity")
            .status,
        WaitStatus::WindowChanged
    );
    let fresh = snapshot_window(fixture.window_id, false, image.to_str()).expect("fresh snapshot");
    press_named(&fixture, "Close");
    let gone = WaitOptions {
        condition: WaitCondition::WindowGone,
        selector: None,
        value: None,
        timeout_ms: 1000,
        interval_ms: 100,
    };
    assert_eq!(
        wait_for_window(&fresh.input_target, &gone)
            .expect("closed window")
            .status,
        WaitStatus::Matched
    );
}
