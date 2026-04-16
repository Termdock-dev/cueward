use crate::doctor::{DoctorCheck, DoctorCheckStatus};

pub(super) const AUTOMATION_FIX: &str =
    "Allow your terminal app in System Settings > Privacy & Security > Automation";
pub(super) const SAFARI_JS_FIX: &str =
    "Enable Safari Develop > Allow JavaScript from Apple Events";

pub(super) fn automation_check(
    id: &'static str,
    target: &'static str,
    result: Result<String, crate::MacosError>,
) -> DoctorCheck {
    match result {
        Ok(message) => DoctorCheck {
            id: id.to_string(),
            status: DoctorCheckStatus::Pass,
            target: target.to_string(),
            message,
            fix: None,
            required: false,
        },
        Err(error) => classify_automation_error(id, target, &error.to_string()),
    }
}

pub(super) fn live_safari_skip_check() -> DoctorCheck {
    DoctorCheck {
        id: "live.safari.js".to_string(),
        status: DoctorCheckStatus::Skip,
        target: "Safari JavaScript probe".to_string(),
        message: "live Safari probe skipped".to_string(),
        fix: None,
        required: false,
    }
}

pub(super) fn live_safari_check(result: Result<String, crate::MacosError>) -> DoctorCheck {
    match result {
        Ok(message) => DoctorCheck {
            id: "live.safari.js".to_string(),
            status: DoctorCheckStatus::Pass,
            target: "Safari JavaScript probe".to_string(),
            message,
            fix: None,
            required: false,
        },
        Err(error) => classify_live_safari_error(&error.to_string()),
    }
}

fn classify_automation_error(id: &'static str, target: &'static str, message: &str) -> DoctorCheck {
    if is_automation_denied(message) {
        DoctorCheck {
            id: id.to_string(),
            status: DoctorCheckStatus::Fail,
            target: target.to_string(),
            message: "automation access denied".to_string(),
            fix: Some(AUTOMATION_FIX.to_string()),
            required: false,
        }
    } else {
        DoctorCheck {
            id: id.to_string(),
            status: DoctorCheckStatus::Warn,
            target: target.to_string(),
            message: message.to_string(),
            fix: None,
            required: false,
        }
    }
}

fn classify_live_safari_error(message: &str) -> DoctorCheck {
    if is_javascript_automation_unavailable(message) {
        DoctorCheck {
            id: "live.safari.js".to_string(),
            status: DoctorCheckStatus::Warn,
            target: "Safari JavaScript probe".to_string(),
            message: "Safari JavaScript automation unavailable".to_string(),
            fix: Some(SAFARI_JS_FIX.to_string()),
            required: false,
        }
    } else if is_automation_denied(message) {
        DoctorCheck {
            id: "live.safari.js".to_string(),
            status: DoctorCheckStatus::Fail,
            target: "Safari JavaScript probe".to_string(),
            message: "automation access denied".to_string(),
            fix: Some(AUTOMATION_FIX.to_string()),
            required: false,
        }
    } else {
        DoctorCheck {
            id: "live.safari.js".to_string(),
            status: DoctorCheckStatus::Warn,
            target: "Safari JavaScript probe".to_string(),
            message: message.to_string(),
            fix: None,
            required: false,
        }
    }
}

fn is_automation_denied(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("not allowed to send apple events")
        || normalized.contains("not authorized to send apple events")
        || normalized.contains("not permitted to send apple events")
        || normalized.contains("permission denied")
        || normalized.contains("1743")
}

fn is_javascript_automation_unavailable(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("javascript from apple events")
        || normalized.contains("allow javascript from apple events")
        || normalized.contains("doesn't understand the do javascript message")
        || normalized.contains("does not understand the do javascript message")
        || normalized.contains("can't make current tab")
}

#[cfg(test)]
mod tests {
    use crate::doctor::DoctorCheckStatus;

    use super::{automation_check, live_safari_check, live_safari_skip_check};

    #[test]
    fn automation_denied_maps_to_fail_with_fix() {
        let check = automation_check(
            "automation.notes",
            "Notes",
            Err(crate::MacosError::Other(
                "osascript: Not authorized to send Apple events to Notes. (-1743)".to_string(),
            )),
        );

        assert_eq!(check.status, DoctorCheckStatus::Fail);
        assert!(check.fix.is_some());
    }

    #[test]
    fn live_safari_skip_check_is_skip() {
        let check = live_safari_skip_check();

        assert_eq!(check.status, DoctorCheckStatus::Skip);
    }

    #[test]
    fn javascript_automation_unavailable_maps_to_warn_with_fix() {
        let check = live_safari_check(Err(crate::MacosError::Other(
            "Safari JavaScript from Apple Events is disabled".to_string(),
        )));

        assert_eq!(check.status, DoctorCheckStatus::Warn);
        assert!(check.fix.is_some());
    }

    #[test]
    fn automation_classifier_does_not_flag_generic_authorization_word() {
        let check = automation_check(
            "automation.notes",
            "Notes",
            Err(crate::MacosError::Other(
                "OAuth authorization code expired".to_string(),
            )),
        );

        assert_eq!(check.status, DoctorCheckStatus::Warn);
        assert!(check.fix.is_none());
    }

    #[test]
    fn javascript_classifier_does_not_flag_generic_do_javascript_with_not_substring() {
        let check = live_safari_check(Err(crate::MacosError::Other(
            "do JavaScript returned notification marker".to_string(),
        )));

        assert_eq!(check.status, DoctorCheckStatus::Warn);
        assert!(check.fix.is_none());
    }
}
