use crate::doctor::DoctorCheck;

use super::shared::probe_target;
use super::targets::{NOTES_ACCOUNTS_ROOT, NOTES_CONTAINER, NOTES_NOTE_STORE};

pub(super) fn run_checks() -> Vec<DoctorCheck> {
    vec![
        probe_target(NOTES_CONTAINER),
        probe_target(NOTES_NOTE_STORE),
        probe_target(NOTES_ACCOUNTS_ROOT),
    ]
}
