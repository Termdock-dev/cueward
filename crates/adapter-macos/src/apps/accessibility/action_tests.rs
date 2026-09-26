use super::{ActionStatus, Target, now};
#[path = "action_test_support.rs"]
mod support;
use support::Fixture;

const FIELD: &str = "w0.0.0";

#[test]
fn app_dispatch_rejects_stale_observations_without_sending_input() {
    let fixture = Fixture::build();
    let mutations: &[(&str, fn(&mut Target))] = &[
        ("application instance changed", |t| t.app.start_seconds += 1),
        ("app context changed", |t| t.context = "0".repeat(64)),
        ("app element changed", |t| {
            t.root_fingerprint = "0".repeat(64)
        }),
        ("app element changed", |t| t.fingerprint = "0".repeat(64)),
        ("target expired", |t| t.issued_at = now().unwrap() - 301),
        ("target expired", |t| t.issued_at = now().unwrap() + 60),
    ];
    for (message, mutate) in mutations {
        let mut target = fixture.observe("valid", FIELD);
        mutate(&mut target);
        for action in ["press", "set_value"] {
            fixture.reject("valid", &target, action, message);
        }
    }
    for action in ["press", "set_value"] {
        let target = fixture.observe("valid", FIELD);
        fixture.reject("receiver-instance", &target, action, "app element changed");
    }
}

#[test]
fn app_dispatch_rechecks_identity_context_and_nodes_after_locking() {
    let fixture = Fixture::build();
    let target = fixture.observe("valid", FIELD);
    for (scenario, message) in [
        ("post-lock-instance", "app context changed"),
        ("post-lock-context", "app context changed"),
        ("post-lock-receiver", "app element changed before dispatch"),
        ("post-lock-node", "app element changed before dispatch"),
        ("post-lock-ancestor", "app element changed before dispatch"),
        ("post-lock-element", "app element changed before dispatch"),
        ("post-lock-screen", "desktop is locked"),
    ] {
        for action in ["press", "set_value"] {
            fixture.reject(scenario, &target, action, message);
        }
    }
}

#[test]
fn app_dispatch_enforces_foreground_caller_and_control_guards() {
    let fixture = Fixture::build();
    let target = fixture.observe("valid", FIELD);
    for scenario in [
        "foreground-host",
        "foreground-receiver",
        "foreground-missing",
        "foreground-zero",
    ] {
        for action in ["press", "set_value"] {
            fixture.reject(scenario, &target, action, "background action stopped");
        }
    }
    for action in ["press", "set_value"] {
        fixture.reject("caller-exited", &target, action, "caller exited");
        let disabled = fixture.observe("disabled", FIELD);
        fixture.reject("disabled", &disabled, action, "element is disabled");
    }
    let unsupported = fixture.observe("unsupported-press", FIELD);
    fixture.reject(
        "unsupported-press",
        &unsupported,
        "press",
        "does not support AXPress",
    );
    let readonly = fixture.observe("readonly", FIELD);
    fixture.reject(
        "readonly",
        &readonly,
        "set_value",
        "does not support text value assignment",
    );
    let menu = fixture.observe("valid", "menu");
    fixture.reject("valid", &menu, "press", "only leaf menu items");
}

#[test]
fn app_dispatch_locks_both_processes_in_order_and_releases_after_failure() {
    let mut fixture = Fixture::build();
    for _ in 0..2 {
        let target = fixture.observe("valid", FIELD);
        let pressed = fixture.accept("valid", &target, "press");
        assert_eq!(pressed.status, ActionStatus::SentUnverified);
        let mut owners = vec![fixture.host, fixture.receiver];
        owners.sort();
        let expected: Vec<_> = owners
            .iter()
            .map(|pid| format!("lock:input-{pid}.lock"))
            .collect();
        assert_eq!(&fixture.events()[..2], expected);
        for owner in owners {
            let held = fixture.hold_lock(owner);
            for action in ["press", "set_value"] {
                fixture.reject("valid", &target, action, "another input action is running");
            }
            drop(held);
            fixture.accept("valid", &target, "press");
        }
        std::mem::swap(&mut fixture.host, &mut fixture.receiver);
    }
    fixture.receiver = fixture.host;
    let target = fixture.observe("valid", FIELD);
    fixture.accept("valid", &target, "press");
    assert_eq!(
        fixture
            .events()
            .iter()
            .filter(|e| e.starts_with("lock:"))
            .count(),
        1
    );
}

#[test]
fn app_dispatch_reports_readback_foreground_changes_and_uncertain_delivery() {
    let fixture = Fixture::build();
    let target = fixture.observe("valid", FIELD);
    let changed = fixture.accept("foreground-changed", &target, "press");
    assert!(changed.foreground_changed);
    assert_eq!(
        fixture.accept("valid", &target, "set_value").status,
        ActionStatus::Confirmed
    );
    assert_eq!(
        fixture.accept("unconfirmed", &target, "set_value").status,
        ActionStatus::SentUnverified
    );
    let menu = fixture.observe("valid", "menu.0");
    assert_eq!(
        fixture.accept("valid", &menu, "press").status,
        ActionStatus::SentUnverified
    );
    for action in ["press", "set_value"] {
        let output = fixture.act("dispatch-error", &target, action);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("delivery is uncertain"));
        assert_eq!(
            fixture
                .events()
                .iter()
                .filter(|e| e.starts_with("dispatch:"))
                .count(),
            1
        );
    }
}
