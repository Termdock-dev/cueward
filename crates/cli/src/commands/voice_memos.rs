use clap::Subcommand;
use std::process;

use super::helpers::print_external;

#[derive(Subcommand)]
pub(crate) enum VoiceMemosAction {
    /// List voice memos
    List,
    /// Read one voice memo by id
    Read {
        /// Voice memo id
        #[arg(long)]
        id: String,
    },
    /// Transcribe one voice memo on this Mac. Needs macOS 26 or later
    Transcribe {
        /// Voice memo id
        #[arg(long)]
        id: String,
        /// Speech locale such as zh_TW. Defaults to the system locale. Must be installed
        #[arg(long)]
        locale: Option<String>,
    },
}

fn transcribe_json(id: &str, locale: Option<&str>) -> Result<String, String> {
    let transcript = cueward_adapter_macos::voice_memos::transcribe::transcribe_voice_memo(id, locale)
        .map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&transcript).map_err(|error| error.to_string())
}

pub(crate) fn dispatch(action: VoiceMemosAction) {
    match action {
        VoiceMemosAction::List => match cueward_adapter_macos::voice_memos::list_voice_memos() {
            Ok(items) => {
                print_external("voice-memos/list", &serde_json::to_string_pretty(&items).unwrap());
                eprintln!("{} voice memo(s)", items.len());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        },
        VoiceMemosAction::Read { id } => {
            match cueward_adapter_macos::voice_memos::read_voice_memo(&id) {
                Ok(item) => {
                    print_external("voice-memos/read", &serde_json::to_string_pretty(&item).unwrap());
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
        VoiceMemosAction::Transcribe { id, locale } => {
            match transcribe_json(&id, locale.as_deref()) {
                Ok(json) => print_external("voice-memos/transcribe", &json),
                Err(e) => {
                    eprintln!("error: {e}");
                    process::exit(1);
                }
            }
        }
    }
}
