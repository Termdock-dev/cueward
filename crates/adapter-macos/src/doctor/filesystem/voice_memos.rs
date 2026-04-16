use crate::doctor::DoctorCheck;

use super::shared::probe_target;
use super::targets::{VOICE_MEMOS_DB, VOICE_MEMOS_ROOT};

pub(super) fn run_checks() -> Vec<DoctorCheck> {
    vec![probe_target(VOICE_MEMOS_ROOT), probe_target(VOICE_MEMOS_DB)]
}
