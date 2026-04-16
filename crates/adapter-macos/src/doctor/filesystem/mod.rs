mod messages;
mod notes;
mod safari;
mod shared;
mod targets;
mod voice_memos;

use super::DoctorCheck;

pub(super) fn run_checks() -> Vec<DoctorCheck> {
    let mut checks = Vec::new();
    checks.extend(messages::run_checks());
    checks.extend(safari::run_checks());
    checks.extend(notes::run_checks());
    checks.extend(voice_memos::run_checks());
    checks
}

#[cfg(test)]
mod tests {
    use super::run_checks;

    #[test]
    fn filesystem_checks_include_expected_ids() {
        let ids: Vec<String> = run_checks().into_iter().map(|check| check.id).collect();

        assert!(ids.contains(&"fda.messages.chat_db".to_string()));
        assert!(ids.contains(&"fda.safari.history_db".to_string()));
        assert!(ids.contains(&"fda.notes.container".to_string()));
        assert!(ids.contains(&"fda.notes.note_store".to_string()));
        assert!(ids.contains(&"fda.notes.accounts_root".to_string()));
        assert!(ids.contains(&"fda.voice_memos.root".to_string()));
        assert!(ids.contains(&"fda.voice_memos.db".to_string()));
    }
}
