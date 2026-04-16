use crate::doctor::DoctorCheck;

use super::shared::probe_target;
use super::targets::SAFARI_HISTORY_DB;

pub(super) fn run_checks() -> Vec<DoctorCheck> {
    vec![probe_target(SAFARI_HISTORY_DB)]
}
