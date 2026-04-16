use std::path::PathBuf;

use crate::doctor::DoctorCheckStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProbeKind {
    Directory,
    Sqlite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProbeTarget {
    pub(super) id: &'static str,
    pub(super) relative_path: &'static str,
    pub(super) kind: ProbeKind,
    pub(super) required: bool,
    pub(super) missing_status: DoctorCheckStatus,
}

impl ProbeTarget {
    pub(super) fn absolute_path(self) -> Result<PathBuf, &'static str> {
        if self.relative_path.starts_with('/') {
            Ok(PathBuf::from(self.relative_path))
        } else {
            Ok(home_dir()?.join(self.relative_path))
        }
    }

    pub(super) fn display_path(self) -> String {
        if self.relative_path.starts_with('/') {
            self.relative_path.to_string()
        } else {
            format!("~/{}", self.relative_path)
        }
    }
}

fn home_dir() -> Result<PathBuf, &'static str> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME environment variable must be set to locate macOS data sources")
}

pub(super) const MESSAGES_CHAT_DB: ProbeTarget = ProbeTarget {
    id: "fda.messages.chat_db",
    relative_path: "Library/Messages/chat.db",
    kind: ProbeKind::Sqlite,
    required: true,
    missing_status: DoctorCheckStatus::Warn,
};

pub(super) const SAFARI_HISTORY_DB: ProbeTarget = ProbeTarget {
    id: "fda.safari.history_db",
    relative_path: "Library/Safari/History.db",
    kind: ProbeKind::Sqlite,
    required: true,
    missing_status: DoctorCheckStatus::Warn,
};

pub(super) const NOTES_CONTAINER: ProbeTarget = ProbeTarget {
    id: "fda.notes.container",
    relative_path: "Library/Group Containers/group.com.apple.notes",
    kind: ProbeKind::Directory,
    required: false,
    missing_status: DoctorCheckStatus::Warn,
};

pub(super) const NOTES_NOTE_STORE: ProbeTarget = ProbeTarget {
    id: "fda.notes.note_store",
    relative_path: "Library/Group Containers/group.com.apple.notes/NoteStore.sqlite",
    kind: ProbeKind::Sqlite,
    required: false,
    missing_status: DoctorCheckStatus::Warn,
};

pub(super) const NOTES_ACCOUNTS_ROOT: ProbeTarget = ProbeTarget {
    id: "fda.notes.accounts_root",
    relative_path: "Library/Group Containers/group.com.apple.notes/Accounts",
    kind: ProbeKind::Directory,
    required: false,
    missing_status: DoctorCheckStatus::Skip,
};

pub(super) const VOICE_MEMOS_ROOT: ProbeTarget = ProbeTarget {
    id: "fda.voice_memos.root",
    relative_path: "Library/Group Containers/group.com.apple.VoiceMemos.shared",
    kind: ProbeKind::Directory,
    required: false,
    missing_status: DoctorCheckStatus::Skip,
};

pub(super) const VOICE_MEMOS_DB: ProbeTarget = ProbeTarget {
    id: "fda.voice_memos.db",
    relative_path: "Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings/CloudRecordings.db",
    kind: ProbeKind::Sqlite,
    required: true,
    missing_status: DoctorCheckStatus::Skip,
};

#[cfg(test)]
mod tests {
    use super::{
        MESSAGES_CHAT_DB, NOTES_ACCOUNTS_ROOT, NOTES_CONTAINER, NOTES_NOTE_STORE,
        SAFARI_HISTORY_DB, VOICE_MEMOS_DB, VOICE_MEMOS_ROOT,
    };

    #[test]
    fn filesystem_target_ids_are_stable() {
        let ids = [
            MESSAGES_CHAT_DB.id,
            SAFARI_HISTORY_DB.id,
            NOTES_CONTAINER.id,
            NOTES_NOTE_STORE.id,
            NOTES_ACCOUNTS_ROOT.id,
            VOICE_MEMOS_ROOT.id,
            VOICE_MEMOS_DB.id,
        ];

        assert_eq!(
            ids,
            [
                "fda.messages.chat_db",
                "fda.safari.history_db",
                "fda.notes.container",
                "fda.notes.note_store",
                "fda.notes.accounts_root",
                "fda.voice_memos.root",
                "fda.voice_memos.db",
            ]
        );
    }
}
