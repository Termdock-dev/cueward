//! One-use same-volume relocation and read-only planning.
use super::{
    MacFiles,
    mutation::{MutationPlatform, supervision},
    policy, protocol,
    store::Store,
};
#[path = "relocation/native.rs"]
mod native;
#[path = "relocation/links.rs"]
mod links;
#[path = "relocation/transport.rs"]
mod transport;
pub use transport::*;
const STORE: Store = Store("files-relocation");
use cueward_core::files::relocation::{self, RelocationPlatform};
pub use cueward_core::files::relocation::{
    RelocationAction, RelocationPlan, RelocationReceipt, RelocationRequest, RelocationStage,
    RelocationStatus,
};
use cueward_core::files::*;
use std::fs::{File, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

impl RelocationPlatform for MacFiles {
    fn open_directory(&self, path: &Path) -> Result<File, FileError> {
        MutationPlatform::open_directory(self, path)
    }
    fn open_symlink(&self, path: &Path) -> Result<File, FileError> {
        links::open(path)
    }
    fn prepare_rename(&self, parent: &Path, receipt: &RelocationReceipt) -> Result<(), FileError> {
        native::prepare(parent, receipt)
    }
    fn rename(&self, root: &File, request: &RelocationRequest) -> relocation::RenameOutcome {
        native::rename(root, request)
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

#[cfg(test)]
#[path = "relocation/execution_tests.rs"]
mod execution_tests;

#[cfg(test)]
#[path = "relocation/race_tests.rs"]
mod race_tests;

#[cfg(test)]
#[path = "relocation/metadata_tests.rs"]
mod mutation_metadata_tests;

#[cfg(test)]
#[path = "relocation/link_tests.rs"]
mod link_tests;
