use clap::Parser;

use super::voice_memos::VoiceMemosAction;
use super::{Cli, Command};

#[test]
fn cli_parses_voice_memos_list() {
    let cli = Cli::try_parse_from(["cueward", "voice-memos", "list"])
        .expect("parse voice memos list");

    match cli.command {
        Command::VoiceMemos {
            action: VoiceMemosAction::List,
        } => {}
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_voice_memos_read() {
    let cli = Cli::try_parse_from([
        "cueward",
        "voice-memos",
        "read",
        "--id",
        "F45D4751-183C-4032-99F7-F1FE1F541BA2",
    ])
    .expect("parse voice memos read");

    match cli.command {
        Command::VoiceMemos {
            action: VoiceMemosAction::Read { id },
        } => {
            assert_eq!(id, "F45D4751-183C-4032-99F7-F1FE1F541BA2");
        }
        _ => panic!("unexpected command"),
    }
}
