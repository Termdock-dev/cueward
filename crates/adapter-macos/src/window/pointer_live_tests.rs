use super::input_live_tests::Receiver;
use super::{InputDelivery, click, drag, input_status};

#[test]
#[ignore = "requires a logged-in macOS desktop, Accessibility and Screen Recording permissions"]
fn pointer_input_routes_clicks_and_drag_to_a_background_non_key_window() {
    let receiver = Receiver::with_source(include_str!("pointer_fixture.swift"), "normal");
    let snapshot = receiver.snapshot(0);
    let status = input_status(&snapshot.input_target).expect("input status");
    assert!(!status.keyboard.dispatch_ready);
    assert!(status.pointer.dispatch_ready);
    assert_eq!(status.application_acceptance, "unverified");
    assert!(
        receiver.state()["events"]
            .as_array()
            .expect("events")
            .is_empty()
    );
    let x = 120.0 * snapshot.image.scale_x;
    let y = 140.0 * snapshot.image.scale_y;
    let clicked = click(&snapshot.input_target, x, y, "left", 2).expect("double click");
    assert_eq!(clicked.status, InputDelivery::SentUnverified);
    click(&snapshot.input_target, x, y, "right", 1).expect("right click");
    let dragged = drag(
        &snapshot.input_target,
        (x, y),
        (
            x + 60.0 * snapshot.image.scale_x,
            y + 40.0 * snapshot.image.scale_y,
        ),
        300,
    )
    .expect("drag");
    assert_eq!(dragged.status, InputDelivery::SentUnverified);
    let state = receiver.wait_for(|s| {
        s["events"]
            .as_array()
            .is_some_and(|e| e.len() == 6 + dragged.events_sent)
    });
    assert_eq!(state["active"], false);
    let events = state["events"].as_array().expect("events");
    assert!(
        events
            .iter()
            .all(|e| e["window"] == snapshot.window.window_id)
    );
    let types: Vec<_> = events
        .iter()
        .map(|e| e["type"].as_u64().expect("type"))
        .collect();
    assert_eq!(&types[..6], &[1, 2, 1, 2, 3, 4]);
    assert_eq!(events[0]["count"], 1);
    assert_eq!(events[2]["count"], 2);
    assert_eq!(types[6], 1);
    assert!(types[7..types.len() - 1].iter().all(|t| *t == 6));
    assert_eq!(types.last(), Some(&2));
    let first = &events[6];
    let last = events.last().expect("release");
    assert_eq!(first["x"], 120.0);
    assert_eq!(last["x"], 180.0);
    assert_eq!(
        last["y"].as_f64().expect("y") - first["y"].as_f64().expect("y"),
        40.0
    );
}

#[test]
#[ignore = "requires a logged-in macOS desktop, Accessibility and Screen Recording permissions"]
fn pointer_drag_releases_at_last_delivered_point_when_window_changes() {
    let receiver = Receiver::with_source(include_str!("pointer_fixture.swift"), "interrupt");
    let snapshot = receiver.snapshot(0);
    let start = (
        100.0 * snapshot.image.scale_x,
        140.0 * snapshot.image.scale_y,
    );
    let end = (
        300.0 * snapshot.image.scale_x,
        240.0 * snapshot.image.scale_y,
    );
    let result = drag(&snapshot.input_target, start, end, 1000).expect("partial drag");
    assert_eq!(result.status, InputDelivery::PartiallySent);
    assert!(result.events_sent >= 3 && result.events_sent < 52);
    let state = receiver.wait_for(|s| {
        s["events"]
            .as_array()
            .is_some_and(|e| e.len() == result.events_sent)
    });
    assert_eq!(state["titles"][0], "Changed pointer fixture");
    let events = state["events"].as_array().expect("events");
    assert_eq!(events[0]["type"], 1);
    let release = &events[events.len() - 1];
    let last_drag = &events[events.len() - 2];
    assert_eq!(release["type"], 2);
    assert_eq!(last_drag["type"], 6);
    assert_eq!(release["x"], last_drag["x"]);
    assert_eq!(release["y"], last_drag["y"]);
}
