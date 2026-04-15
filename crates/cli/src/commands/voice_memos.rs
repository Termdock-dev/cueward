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
    }
}
