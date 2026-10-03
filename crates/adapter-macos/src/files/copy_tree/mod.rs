//! Read-only tree observations and separate supervised no-overwrite execution.
mod native;
pub mod execution;
use super::{MacFiles, policy, protocol};
use cueward_core::files::FileError;
use cueward_core::files::copy_tree::{self, CopyTreePlan, CopyTreeRequest};
use std::path::Path;

/// Run bounded directory planning under one whole-worker deadline.
pub fn run_plan(
    executable: &Path,
    request: &CopyTreeRequest,
    timeout: u64,
) -> Result<CopyTreePlan, FileError> {
    protocol::run_typed(executable, "files-copy-tree-plan-worker", request, timeout)
}

/// Deny cloud materialization before any scoped directory/metadata probes.
pub fn plan_worker(request: &CopyTreeRequest) -> Result<CopyTreePlan, FileError> {
    protocol::payload(request, 10000)?;
    let _policy = policy::NoMaterialization::enter()?;
    copy_tree::plan(&MacFiles, request)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod safety_tests;

#[cfg(test)]
mod package_tests;
