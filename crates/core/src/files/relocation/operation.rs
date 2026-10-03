use super::*;
use context::Context;

/// Execute once with checkpointed evidence; never overwrite, permanently delete or rollback.
pub fn execute<P: RelocationPlatform>(
    platform: &P,
    receipt: &mut RelocationReceipt,
    checkpoint: &mut impl FnMut(&RelocationReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    validation::validate(&receipt.request)?;
    let context = Context::prepare(platform, &receipt.request)?;
    receipt.before = Some(build_plan(platform, &receipt.request, &context)?);
    let before = receipt
        .before
        .as_ref()
        .ok_or_else(|| invalid("missing observations"))?;
    if !before.same_filesystem {
        if before.destination_before.is_some() {
            return Err(FileError::new(
                FileErrorCode::Conflict,
                "destination exists; nothing overwritten",
            ));
        }
        return platform.cross_volume_move(receipt, checkpoint);
    }
    let no_op = prepare(platform, receipt)?;
    receipt.stage = RelocationStage::Prepared;
    receipt.renamed = Some(false);
    checkpoint(receipt)?;
    context.revalidate(&receipt.request)?;
    if !no_op {
        if let Err(error) = submit(platform, &context, receipt, checkpoint) {
            // Best-effort actual path evidence, without hiding the submission error.
            let _ = verification::observe(platform, receipt);
            return Err(error);
        }
    }
    verification::verify(platform, &context, receipt, no_op)?;
    receipt.status = RelocationStatus::Completed;
    receipt.stage = RelocationStage::Finished;
    receipt.completion_verified = true;
    checkpoint(receipt)
}

fn submit<P: RelocationPlatform>(
    platform: &P,
    context: &Context<'_, P>,
    receipt: &mut RelocationReceipt,
    checkpoint: &mut impl FnMut(&RelocationReceipt) -> Result<(), FileError>,
) -> Result<(), FileError> {
    receipt.stage = RelocationStage::Renaming;
    receipt.mutation_attempted = true;
    receipt.renamed = None;
    if let Err(error) = checkpoint(receipt) {
        receipt.mutation_attempted = false;
        receipt.renamed = Some(false);
        return Err(error);
    }
    // This narrows the race window but is not a kernel source-identity condition.
    if let Err(error) = context.revalidate(&receipt.request) {
        receipt.mutation_attempted = false;
        receipt.renamed = Some(false);
        return Err(error);
    }
    let outcome = platform.rename(&context.root, &receipt.request);
    receipt.renamed = outcome.renamed;
    receipt.stage = RelocationStage::Verifying;
    checkpoint(receipt)?;
    outcome.result
}
fn invalid(message: &str) -> FileError {
    FileError::new(FileErrorCode::InvalidOptions, message)
}

fn prepare(
    platform: &impl RelocationPlatform,
    receipt: &RelocationReceipt,
) -> Result<bool, FileError> {
    let before = receipt
        .before
        .as_ref()
        .ok_or_else(|| invalid("missing observations"))?;
    let no_op = before.no_op;
    if !no_op && before.destination_before.is_some() {
        return Err(FileError::new(
            FileErrorCode::Conflict,
            "destination already exists; nothing overwritten",
        ));
    }
    if !before.same_filesystem {
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            "cross-filesystem relocation is unsupported; no copy/delete fallback",
        ));
    }
    if !no_op {
        platform.prepare_rename(Path::new(&before.destination_parent.path), receipt)?;
    }
    Ok(no_op)
}
