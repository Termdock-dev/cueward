//! Bounded read-only batch rename observations; no operation store or native rename calls.
use super::{MacFiles, policy, protocol};
use cueward_core::files::FileError;
use cueward_core::files::relocation::batch::{self, BatchRenamePlatform};
pub use cueward_core::files::relocation::batch::{
    BatchRenameEntry, BatchRenameIssueCode, BatchRenamePlan, BatchRenameRequest,
};
use objc2_foundation::{NSComparisonResult, NSString};
use std::path::Path;

impl BatchRenamePlatform for MacFiles {
    fn names_may_collide(&self, left: &str, right: &str) -> bool {
        let a = NSString::from_str(left).decomposedStringWithCanonicalMapping();
        let b = NSString::from_str(right).decomposedStringWithCanonicalMapping();
        a.caseInsensitiveCompare(&b) == NSComparisonResult::Same
    }
}

/// Share one deadline/request limit with existing read-only file workers.
pub fn run_plan(
    executable: &Path,
    request: &BatchRenameRequest,
    timeout: u64,
) -> Result<BatchRenamePlan, FileError> {
    protocol::run_typed(
        executable,
        "files-batch-rename-plan-worker",
        request,
        timeout,
    )
}

/// Deny placeholder materialization, then observe without allocating mutation receipts.
pub fn plan_worker(request: &BatchRenameRequest) -> Result<BatchRenamePlan, FileError> {
    protocol::payload(request, 10000)?;
    let _policy = policy::NoMaterialization::enter()?;
    batch::plan(&MacFiles, request)
}

#[cfg(test)]
#[path = "batch_rename_tests.rs"]
mod tests;
