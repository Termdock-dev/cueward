use crate::MacosError;

/// Create a reminder in Apple Reminders.
pub fn create_reminder(title: &str, notes: &str, list: &str) -> Result<(), MacosError> {
    crate::reminders::create_reminder(title, notes, list, None, None)
}
