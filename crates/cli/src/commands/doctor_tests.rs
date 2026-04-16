use clap::Parser;

use cueward_adapter_macos::doctor::{DoctorCheck, DoctorCheckStatus, DoctorReport};

use super::doctor::{exit_code_for_report, render_human_report};
use super::{Cli, Command};

#[test]
fn cli_parses_doctor_default() {
    let cli = Cli::try_parse_from(["cueward", "doctor"]).expect("parse doctor");

    match cli.command {
        Command::Doctor { json, live_safari } => {
            assert!(!json);
            assert!(!live_safari);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_doctor_json() {
    let cli = Cli::try_parse_from(["cueward", "doctor", "--json"]).expect("parse doctor json");

    match cli.command {
        Command::Doctor { json, live_safari } => {
            assert!(json);
            assert!(!live_safari);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_doctor_live_safari() {
    let cli = Cli::try_parse_from(["cueward", "doctor", "--live-safari"])
        .expect("parse doctor live safari");

    match cli.command {
        Command::Doctor { json, live_safari } => {
            assert!(!json);
            assert!(live_safari);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_doctor_json_and_live_safari() {
    let cli = Cli::try_parse_from(["cueward", "doctor", "--json", "--live-safari"])
        .expect("parse doctor json live safari");

    match cli.command {
        Command::Doctor { json, live_safari } => {
            assert!(json);
            assert!(live_safari);
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn doctor_human_report_includes_message_and_fix() {
    let report = DoctorReport {
        ok: false,
        checks: vec![DoctorCheck {
            id: "automation.notes".to_string(),
            status: DoctorCheckStatus::Fail,
            target: "Notes".to_string(),
            message: "permission denied".to_string(),
            fix: Some("Grant Automation access".to_string()),
            required: true,
        }],
    };

    let rendered = render_human_report(&report);

    assert!(rendered.contains("permission denied"));
    assert!(rendered.contains("Grant Automation access"));
}

#[test]
fn doctor_exit_code_is_non_zero_when_report_is_not_ok() {
    let failing = DoctorReport {
        ok: false,
        checks: Vec::new(),
    };
    let passing = DoctorReport {
        ok: true,
        checks: Vec::new(),
    };

    assert_eq!(exit_code_for_report(&failing), 1);
    assert_eq!(exit_code_for_report(&passing), 0);
}
