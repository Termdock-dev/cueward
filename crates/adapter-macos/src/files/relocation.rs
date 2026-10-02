//! Metadata-only preparation shared by possible future relocation workflows.
use super::{MacFiles, mutation::MutationPlatform, policy};
use cueward_core::files::relocation::{self, RelocationPlatform};
pub use cueward_core::files::relocation::{RelocationAction, RelocationPlan, RelocationRequest};
use cueward_core::files::*;
use std::fs::{File, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

impl RelocationPlatform for MacFiles {
    fn open_directory(&self, path: &Path) -> Result<File, FileError> {
        MutationPlatform::open_directory(self, path)
    }
    fn same_filesystem(&self, source: &Metadata, destination_parent: &Metadata) -> bool {
        source.dev() == destination_parent.dev()
    }
}

/// Observe proposed names under the existing no-materialization policy; never rename them.
pub fn plan_worker(request: &RelocationRequest) -> Result<RelocationPlan, FileError> {
    let _policy = policy::NoMaterialization::enter()?;
    relocation::plan(&MacFiles, request)
}

#[cfg(test)]
#[path = "relocation_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "relocation_safety_tests.rs"]
mod safety_tests;
