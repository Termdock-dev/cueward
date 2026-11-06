use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{VoiceMemoItem, read_voice_memo};
use crate::MacosError;
use crate::window::process::run_with_timeout;

const HELPER_SCRIPT: &str = include_str!("../../scripts/voice_memo_transcribe.swift");

/// Covers a cold Swift compile and model load, which the audio length does not predict.
const HELPER_STARTUP: Duration = Duration::from_secs(120);

/// Allows four times real speed. The helper measured about 65 times real speed on an M4 Pro.
const HELPER_SECONDS_PER_AUDIO_SECOND: f64 = 0.25;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VoiceMemoTranscript {
    pub id: String,
    pub title: String,
    pub locale: String,
    pub text: String,
}

#[derive(Debug, PartialEq)]
struct Transcription {
    locale: String,
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum HelperResult {
    Transcribed { locale: String, text: String },
    NoSpeech { locale: String },
    UnsupportedLocale { locale: String, installed: Vec<String> },
    LocaleNotInstalled { locale: String, installed: Vec<String> },
    Error { message: String },
}

/// Transcribe one voice memo on this Mac without downloading speech assets.
pub fn transcribe_voice_memo(
    id: &str,
    locale: Option<&str>,
) -> Result<VoiceMemoTranscript, MacosError> {
    let item = read_voice_memo(id)?;
    let path = local_audio_path(&item)?;
    let transcription =
        transcribe_audio_file(path, locale, helper_timeout(item.duration_seconds))?;
    Ok(VoiceMemoTranscript {
        id: item.id,
        title: item.title,
        locale: transcription.locale,
        text: transcription.text,
    })
}

fn local_audio_path(item: &VoiceMemoItem) -> Result<&Path, MacosError> {
    item.path.as_deref().map(Path::new).ok_or_else(|| {
        MacosError::Other(format!("voice memo has no local audio file: {}", item.id))
    })
}

fn helper_timeout(duration_seconds: f64) -> Duration {
    let audio =
        Duration::try_from_secs_f64(duration_seconds * HELPER_SECONDS_PER_AUDIO_SECOND)
            .unwrap_or_default();
    HELPER_STARTUP.saturating_add(audio)
}

fn transcribe_audio_file(
    path: &Path,
    locale: Option<&str>,
    timeout: Duration,
) -> Result<Transcription, MacosError> {
    let mut script = tempfile::NamedTempFile::with_suffix(".swift")
        .map_err(|error| MacosError::Other(format!("failed to create speech script: {error}")))?;
    script
        .write_all(HELPER_SCRIPT.as_bytes())
        .map_err(|error| MacosError::Other(format!("failed to write speech script: {error}")))?;
    let mut command = Command::new("swift");
    command.arg(script.path()).arg(path).args(locale);
    let output = run_with_timeout(&mut command, &[], timeout)
        .map_err(|error| helper_run_error(&error, timeout))?;
    parse_output(&output)
}

fn helper_run_error(error: &io::Error, timeout: Duration) -> MacosError {
    if error.kind() == io::ErrorKind::TimedOut {
        MacosError::Other(format!(
            "transcription exceeded its {} second limit and was stopped",
            timeout.as_secs()
        ))
    } else {
        MacosError::Other(format!("failed to run swift speech helper: {error}"))
    }
}

fn parse_output(output: &Output) -> Result<Transcription, MacosError> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MacosError::Other(format!(
            "speech helper failed: {}. On-device transcription needs macOS 26 or later",
            if stderr.trim().is_empty() {
                "helper exited without a result"
            } else {
                stderr.trim()
            }
        )));
    }
    let result = serde_json::from_slice(&output.stdout)
        .map_err(|error| MacosError::Other(format!("invalid speech helper result: {error}")))?;
    interpret_result(result)
}

fn interpret_result(result: HelperResult) -> Result<Transcription, MacosError> {
    match result {
        HelperResult::Transcribed { locale, text } => Ok(Transcription { locale, text }),
        HelperResult::NoSpeech { locale } => Err(MacosError::Other(format!(
            "no speech detected with locale {locale}; no transcript was produced"
        ))),
        HelperResult::UnsupportedLocale { locale, installed } => Err(MacosError::Other(format!(
            "locale {locale} is not supported for transcription; installed locales: {}",
            locale_list(&installed)
        ))),
        HelperResult::LocaleNotInstalled { locale, installed } => Err(MacosError::Other(format!(
            "speech assets for locale {locale} are not installed and cueward does not download them; installed locales: {}",
            locale_list(&installed)
        ))),
        HelperResult::Error { message } => {
            Err(MacosError::Other(format!("transcription failed: {message}")))
        }
    }
}

fn locale_list(installed: &[String]) -> String {
    if installed.is_empty() {
        "none".into()
    } else {
        installed.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    use super::*;

    fn helper_output(exit_status: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(exit_status << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    fn parse_error(stdout: &str) -> String {
        parse_output(&helper_output(0, stdout, ""))
            .expect_err("helper result should be an error")
            .to_string()
    }

    #[test]
    fn transcribed_result_returns_locale_and_text() {
        let output = helper_output(
            0,
            r#"{"status":"transcribed","locale":"zh_TW","text":"今天開會"}"#,
            "",
        );

        assert_eq!(
            parse_output(&output).expect("transcript"),
            Transcription {
                locale: "zh_TW".into(),
                text: "今天開會".into(),
            }
        );
    }

    #[test]
    fn no_speech_is_an_error_and_never_an_empty_transcript() {
        let message = parse_error(r#"{"status":"no_speech","locale":"zh_TW"}"#);

        assert_eq!(
            message,
            "no speech detected with locale zh_TW; no transcript was produced"
        );
    }

    #[test]
    fn unsupported_locale_names_installed_locales() {
        let message = parse_error(
            r#"{"status":"unsupported_locale","locale":"xx_YY","installed":["zh_CN","zh_TW"]}"#,
        );

        assert!(message.contains("xx_YY"), "{message}");
        assert!(message.ends_with("installed locales: zh_CN, zh_TW"), "{message}");
    }

    #[test]
    fn missing_assets_name_installed_locales_and_refuse_to_download() {
        let message = parse_error(
            r#"{"status":"locale_not_installed","locale":"en_US","installed":["zh_TW"]}"#,
        );

        assert!(message.contains("en_US"), "{message}");
        assert!(message.contains("does not download"), "{message}");
        assert!(message.ends_with("installed locales: zh_TW"), "{message}");
    }

    #[test]
    fn empty_installed_list_is_named_as_none() {
        let message = parse_error(
            r#"{"status":"locale_not_installed","locale":"en_US","installed":[]}"#,
        );

        assert!(message.ends_with("installed locales: none"), "{message}");
    }

    #[test]
    fn helper_error_result_keeps_its_message() {
        let message = parse_error(r#"{"status":"error","message":"cannot open audio"}"#);

        assert_eq!(message, "transcription failed: cannot open audio");
    }

    #[test]
    fn failed_helper_reports_stderr() {
        let error = parse_output(&helper_output(1, "", "error: no such module 'Speech'\n"))
            .expect_err("failed helper");

        assert!(error.to_string().contains("no such module 'Speech'"), "{error}");
    }

    #[test]
    fn failed_helper_without_stderr_still_explains_itself() {
        let error = parse_output(&helper_output(1, "", "")).expect_err("failed helper");

        assert!(error.to_string().contains("exited without a result"), "{error}");
    }

    #[test]
    fn unreadable_helper_output_is_rejected() {
        for stdout in ["", "TEXT: hello", r#"{"status":"later"}"#] {
            let message = parse_error(stdout);

            assert!(message.starts_with("invalid speech helper result"), "{message}");
        }
    }

    #[test]
    fn timeout_error_replaces_the_shared_runner_wording() {
        let error = io::Error::new(io::ErrorKind::TimedOut, "AX helper timed out");

        let message = helper_run_error(&error, Duration::from_secs(150)).to_string();

        assert_eq!(message, "transcription exceeded its 150 second limit and was stopped");
    }

    #[test]
    fn timeout_grows_with_the_recording_length() {
        assert_eq!(helper_timeout(0.0), HELPER_STARTUP);
        assert_eq!(helper_timeout(7200.0), Duration::from_secs(120 + 1800));
    }

    #[test]
    fn timeout_ignores_invalid_durations() {
        for duration in [-1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(helper_timeout(duration), HELPER_STARTUP);
        }
    }

    #[test]
    fn memo_without_local_file_is_rejected_before_the_helper_runs() {
        let item = VoiceMemoItem {
            id: "memo-1".into(),
            title: "Title".into(),
            duration_seconds: 1.0,
            timestamp: "2026-11-07T00:00:00+08:00".into(),
            path: None,
        };

        let error = local_audio_path(&item).expect_err("missing file");

        assert_eq!(error.to_string(), "voice memo has no local audio file: memo-1");
    }

    fn run_tool(tool: &str, arguments: &[&std::ffi::OsStr]) {
        let status = Command::new(tool).args(arguments).status().expect(tool);
        assert!(status.success(), "{tool} failed");
    }

    #[test]
    #[ignore = "runs the Speech framework on synthetic Meijia audio"]
    fn live_transcribes_synthetic_meijia_audio() {
        let temp = tempfile::tempdir().expect("tempdir");
        let aiff = temp.path().join("memo.aiff");
        let m4a = temp.path().join("memo.m4a");
        run_tool(
            "say",
            &[
                "-v".as_ref(),
                "Meijia".as_ref(),
                "-o".as_ref(),
                aiff.as_os_str(),
                "今天下午三點在台北開會，討論明年的預算。".as_ref(),
            ],
        );
        run_tool(
            "afconvert",
            &[
                "-f".as_ref(),
                "m4af".as_ref(),
                "-d".as_ref(),
                "aac".as_ref(),
                aiff.as_os_str(),
                m4a.as_os_str(),
            ],
        );

        let transcription =
            transcribe_audio_file(&m4a, Some("zh_TW"), helper_timeout(10.0)).expect("transcript");

        println!("{transcription:?}");
        assert_eq!(transcription.locale, "zh_TW");
        assert!(transcription.text.contains("預算"), "{}", transcription.text);
    }
}
