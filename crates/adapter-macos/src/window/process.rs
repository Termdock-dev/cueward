use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct HelperProcess(Child);

impl HelperProcess {
    fn terminate(&mut self) -> io::Result<()> {
        // The Swift driver can spawn a compiler. Stop the whole owned process group.
        let group = Command::new("/bin/kill")
            .args(["-KILL", "--", &format!("-{}", self.0.id())])
            .output();
        let _ = self.0.kill();
        self.0.wait()?;
        match group {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(io::Error::other(format!(
                "failed to stop AX helper process group: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))),
            Err(error) => Err(error),
        }
    }
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        // A reaped child needs no cleanup; all other error paths stop the helper.
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.terminate();
        }
    }
}

fn read_output(mut file: File) -> io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub(super) fn run_with_timeout(
    command: &mut Command,
    payload: &[u8],
    timeout: Duration,
) -> io::Result<Output> {
    // Files avoid blocking on full stdin/stdout pipes while the driver is compiling.
    let mut input = tempfile::tempfile()?;
    input.write_all(payload)?;
    input.seek(SeekFrom::Start(0))?;
    let stdout = tempfile::tempfile()?;
    let stderr = tempfile::tempfile()?;
    let deadline = Instant::now() + timeout;
    let mut child = HelperProcess(
        command
            .process_group(0)
            .stdin(Stdio::from(input))
            .stdout(Stdio::from(stdout.try_clone()?))
            .stderr(Stdio::from(stderr.try_clone()?))
            .spawn()?,
    );
    loop {
        if let Some(status) = child.0.try_wait()? {
            return Ok(Output {
                status,
                stdout: read_output(stdout)?,
                stderr: read_output(stderr)?,
            });
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            let cleanup = child.terminate();
            let detail = cleanup
                .err()
                .map(|error| format!("; cleanup failed: {error}"))
                .unwrap_or_default();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "AX helper timed out{detail}; an action may have been delivered; inspect before retrying"
                ),
            ));
        }
        thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
