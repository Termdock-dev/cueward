use crate::applescript::run_capture;
use crate::doctor::DoctorCheck;

use super::classify::automation_check;

const NOTES_SCRIPT: &str = r#"
tell application "Notes"
    return "notes access ok"
end tell
"#;

const REMINDERS_SCRIPT: &str = r#"
tell application "Reminders"
    return "reminders access ok"
end tell
"#;

const CALENDAR_SCRIPT: &str = r#"
tell application "Calendar"
    return "calendar access ok"
end tell
"#;

const SAFARI_SCRIPT: &str = r#"
tell application "Safari"
    return "safari access ok"
end tell
"#;

pub(super) fn run_checks() -> Vec<DoctorCheck> {
    vec![
        automation_check(
            "automation.notes",
            "Notes",
            run_capture(NOTES_SCRIPT, "doctor_automation_notes")
                .map(|output| output.trim().to_string()),
        ),
        automation_check(
            "automation.reminders",
            "Reminders",
            run_capture(REMINDERS_SCRIPT, "doctor_automation_reminders")
                .map(|output| output.trim().to_string()),
        ),
        automation_check(
            "automation.calendar",
            "Calendar",
            run_capture(CALENDAR_SCRIPT, "doctor_automation_calendar")
                .map(|output| output.trim().to_string()),
        ),
        automation_check(
            "automation.safari",
            "Safari",
            run_capture(SAFARI_SCRIPT, "doctor_automation_safari")
                .map(|output| output.trim().to_string()),
        ),
    ]
}
