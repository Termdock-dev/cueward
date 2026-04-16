use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorCheckStatus {
    Pass,
    Fail,
    Warn,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub id: String,
    pub status: DoctorCheckStatus,
    pub target: String,
    pub message: String,
    pub fix: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub checks: Vec<DoctorCheck>,
}

#[cfg(test)]
mod tests {
    use super::{DoctorCheck, DoctorCheckStatus, DoctorReport};

    #[test]
    fn doctor_check_status_serializes_as_lowercase() {
        let value = serde_json::to_string(&DoctorCheckStatus::Pass).expect("serialize status");

        assert_eq!(value, "\"pass\"");
    }

    #[test]
    fn doctor_report_serializes_stable_fields() {
        let report = DoctorReport {
            ok: false,
            checks: vec![DoctorCheck {
                id: "fda.messages.chat_db".to_string(),
                status: DoctorCheckStatus::Fail,
                target: "~/Library/Messages/chat.db".to_string(),
                message: "permission denied".to_string(),
                fix: None,
                required: true,
            }],
        };

        let value = serde_json::to_value(&report).expect("serialize report");
        let expected = serde_json::json!({
            "ok": false,
            "checks": [{
                "id": "fda.messages.chat_db",
                "status": "fail",
                "target": "~/Library/Messages/chat.db",
                "message": "permission denied",
                "fix": null,
                "required": true
            }]
        });

        assert_eq!(value, expected);
    }
}
