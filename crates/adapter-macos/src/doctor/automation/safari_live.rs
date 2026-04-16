use crate::doctor::DoctorCheck;

use super::classify::{live_safari_check, live_safari_skip_check};

pub(super) fn run_check(live_safari: bool) -> DoctorCheck {
    if !live_safari {
        return live_safari_skip_check();
    }

    live_safari_check(crate::safari::doctor_live_probe())
}
