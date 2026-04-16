mod apps;
mod classify;
mod safari_live;

use super::DoctorCheck;

pub(super) fn run_checks(live_safari: bool) -> Vec<DoctorCheck> {
    let mut checks = apps::run_checks();
    checks.push(safari_live::run_check(live_safari));
    checks
}
