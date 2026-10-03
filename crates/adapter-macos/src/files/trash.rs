//! Read-only trash-target observations under the existing no-materialization policy.
use super::{MacFiles, policy, protocol};
use cueward_core::files::{FileError, trash};
use std::path::Path;
pub use trash::{TrashPlan, TrashPlanRequest};

/// Supervise the whole metadata-only worker; no mutation receipt is allocated.
pub fn run_plan(
    executable: &Path,
    request: &TrashPlanRequest,
    timeout: u64,
) -> Result<TrashPlan, FileError> {
    protocol::run_typed(executable, "files-trash-plan-worker", request, timeout)
}
/// Observe scoped entry metadata without native trashing, link resolution or downloads.
pub fn plan_worker(request: &TrashPlanRequest) -> Result<TrashPlan, FileError> {
    protocol::payload(request, 10000)?;
    let _policy = policy::NoMaterialization::enter()?;
    trash::plan(&MacFiles, request)
}
#[cfg(test)]
#[path = "trash_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "trash_safety_tests.rs"]
mod safety_tests;
