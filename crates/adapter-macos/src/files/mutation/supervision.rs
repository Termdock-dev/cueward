//! A parent-owned stdin socket is both framed input and a fail-closed worker lifeline.
use super::*;
use std::io::{self, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Publish only the generated recovery ID, failing before dispatch if stderr cannot be flushed.
pub(super) fn announce(id: &str) -> Result<(), FileError> {
    // Only a generated UUID is printed here, never unwrapped external paths/content.
    let mut stderr = io::stderr().lock();
    writeln!(
        stderr,
        "cueward files operation_id={id}; inspect with: cueward files receipt --operation-id {id}"
    )?;
    stderr.flush()?;
    Ok(())
}

/// Keep the parent socket open while the shared supervisor owns the mutation worker.
pub(super) fn run(
    executable: &Path,
    request: &MutationWorkerRequest,
    timeout_ms: u64,
) -> Result<MutationReceipt, FileError> {
    let mut payload = protocol::payload(request, timeout_ms)?;
    payload.push(b'\n');
    let (mut parent, worker) = UnixStream::pair()?;
    parent.write_all(&payload)?;
    // Do not shut down the write half: EOF means parent cancellation/death, not input end.
    // Both originals are CLOEXEC; only the worker endpoint becomes its stdin on spawn.
    let result = crate::window::process::run_with_input(
        Command::new(executable).arg("files-mutation-worker"),
        Stdio::from(OwnedFd::from(worker)),
        Duration::from_millis(timeout_ms),
    )
    .map_err(protocol::transport_error)
    .and_then(protocol::decode_output);
    drop(parent);
    result
}

/// Read one bounded JSON line, then monitor stdin for the rest of the worker lifetime.
pub(super) fn read_request() -> Result<MutationWorkerRequest, FileError> {
    let mut bytes = Vec::new();
    {
        let mut stdin = io::stdin().lock();
        loop {
            let mut byte = [0];
            if stdin.read(&mut byte)? == 0 {
                return Err(invalid(
                    "mutation worker input requires a newline and a live parent",
                ));
            }
            if byte[0] == b'\n' {
                break;
            }
            bytes.push(byte[0]);
            if bytes.len() > protocol::MAX_REQUEST_BYTES {
                return Err(invalid("mutation worker input exceeds 16 KiB"));
            }
        }
    }
    let request = serde_json::from_slice(&bytes).map_err(|e| invalid(&e.to_string()))?;
    arm_monitor()?;
    Ok(request)
}

fn arm_monitor() -> Result<(), FileError> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(0);
    std::thread::Builder::new()
        .name("mutation-parent-lifetime".into())
        .spawn(move || {
            // Check EOF before allowing the mutation thread to proceed. This also covers
            // parent death before worker exec/startup. No signal handlers or PID reuse.
            let result = check_parent();
            let ready = result.is_ok();
            if sender.send(result).is_err() || !ready {
                return;
            }
            loop {
                let mut byte = [0];
                match io::stdin().read(&mut byte) {
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    // EOF, unexpected data or transport errors all stop the entire worker,
                    // including a mutation thread blocked in native filesystem operations.
                    _ => unsafe { _exit(125) },
                }
            }
        })?;
    receiver
        .recv()
        .map_err(|_| FileError::new(FileErrorCode::Internal, "parent monitor failed to start"))?
}

fn check_parent() -> Result<(), FileError> {
    let mut descriptor = PollFd {
        fd: 0,
        events: 0x0001, // POLLIN, from Darwin sys/poll.h.
        revents: 0,
    };
    loop {
        let result = unsafe { poll(&mut descriptor, 1, 0) };
        if result == 0 {
            return Ok(());
        }
        if result < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
            continue;
        }
        return Err(FileError::new(
            FileErrorCode::Unavailable,
            "mutation worker parent is gone or its lifeline is invalid",
        ));
    }
}
fn invalid(message: &str) -> FileError {
    FileError::new(FileErrorCode::InvalidOptions, message)
}
#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}
unsafe extern "C" {
    fn poll(fds: *mut PollFd, count: u32, timeout: i32) -> i32;
    fn _exit(status: i32) -> !;
}
