use cueward_core::files::*;
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

pub const MAX_REQUEST_BYTES: usize = 16384;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const PREFIX: &str = "<external source=\"cueward/files/worker\">\n";
const SUFFIX: &str = "\n</external>\n";

/// Run the dedicated read-only worker with a bounded request and deadline.
pub fn run(
    executable: &Path,
    request: &FileRequest,
    timeout_ms: u64,
) -> Result<FileResponse, FileError> {
    run_typed(executable, "files-worker", request, timeout_ms)
}

/// Share bounded process supervision and external JSON across file workers.
pub(super) fn run_typed<T: DeserializeOwned>(
    executable: &Path,
    worker: &str,
    request: &impl Serialize,
    timeout_ms: u64,
) -> Result<T, FileError> {
    let payload = payload(request, timeout_ms)?;
    let output = crate::window::process::run_with_timeout(
        Command::new(executable).arg(worker),
        &payload,
        Duration::from_millis(timeout_ms),
    )
    .map_err(transport_error)?;
    decode_output(output)
}

/// Preserve structured transport errors across read-only and mutation supervision.
pub(super) fn transport_error(error: std::io::Error) -> FileError {
    if error.kind() == std::io::ErrorKind::TimedOut {
        FileError::new(
            FileErrorCode::Timeout,
            "file worker exceeded its deadline and was stopped",
        )
    } else {
        FileError::from(error)
    }
}

/// Validate the external response envelope and agreement with the process exit status.
pub(super) fn decode_output<T: DeserializeOwned>(output: Output) -> Result<T, FileError> {
    let result = parse(&output.stdout)?;
    if output.status.success() != result.is_ok() {
        return Err(FileError::new(
            FileErrorCode::Internal,
            "worker exit status disagrees with its result",
        ));
    }
    result
}

/// Serialize a bounded request after validating the worker deadline.
pub(super) fn payload(request: &impl Serialize, timeout_ms: u64) -> Result<Vec<u8>, FileError> {
    if !(1..=30000).contains(&timeout_ms) {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "timeout-ms must be 1..30000",
        ));
    }
    let payload = serde_json::to_vec(request).map_err(internal)?;
    if payload.len() > MAX_REQUEST_BYTES {
        return Err(FileError::new(
            FileErrorCode::InvalidOptions,
            "request exceeds 16 KiB",
        ));
    }
    Ok(payload)
}

fn internal(error: impl std::fmt::Display) -> FileError {
    FileError::new(FileErrorCode::Internal, error.to_string())
}

fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<Result<T, FileError>, FileError> {
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(internal("worker response exceeds 16 MiB"));
    }
    let text = std::str::from_utf8(bytes).map_err(internal)?;
    let payload = text
        .strip_prefix(PREFIX)
        .and_then(|t| t.strip_suffix(SUFFIX))
        .ok_or_else(|| internal("worker returned an invalid external envelope"))?;
    serde_json::from_str(payload).map_err(internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn blocking_worker_is_stopped_at_the_deadline() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("worker");
        std::fs::write(&executable, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let request = FileRequest {
            root: directory.path().to_owned(),
            path: ".".into(),
            follow_links: false,
            expected_version: None,
            action: FileAction::Info,
        };
        let start = std::time::Instant::now();
        assert_eq!(
            run(&executable, &request, 100).unwrap_err().code,
            FileErrorCode::Timeout
        );
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn strict_protocol_rejects_partial_or_unwrapped_outputs() {
        for value in [
            "",
            "{}",
            "<external>\n{}\n</external>\n",
            "<external source=\"cueward/files/worker\">\n{}",
        ] {
            assert_eq!(
                parse::<FileResponse>(value.as_bytes()).unwrap_err().code,
                FileErrorCode::Internal
            );
        }
        let payload = format!(
            "{PREFIX}{{\"Err\":{{\"code\":\"permission_denied\",\"message\":\"denied\"}}}}{SUFFIX}"
        );
        assert_eq!(
            parse::<FileResponse>(payload.as_bytes())
                .unwrap()
                .unwrap_err()
                .code,
            FileErrorCode::PermissionDenied
        );
    }
}
