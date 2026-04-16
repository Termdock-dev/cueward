use crate::doctor::DoctorCheck;

use super::shared::probe_target;
use super::targets::MESSAGES_CHAT_DB;

pub(super) fn run_checks() -> Vec<DoctorCheck> {
    vec![probe_target(MESSAGES_CHAT_DB)]
}
