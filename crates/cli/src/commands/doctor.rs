use std::process;

use cueward_adapter_macos::doctor::{
    DoctorCheck, DoctorCheckStatus, DoctorOptions, DoctorReport, run_doctor,
};

pub(crate) fn dispatch(json: bool, live_safari: bool) {
    let report = run_doctor(DoctorOptions { live_safari });

    if json {
        match serde_json::to_string_pretty(&report) {
            Ok(serialized) => println!("{serialized}"),
            Err(error) => {
                eprintln!("error: failed to serialize doctor report: {error}");
                process::exit(1);
            }
        }
    } else {
        println!("{}", render_human_report(&report));
    }

    let exit_code = exit_code_for_report(&report);
    if exit_code != 0 {
        process::exit(exit_code);
    }
}

pub(crate) fn render_human_report(report: &DoctorReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!("doctor: {}", if report.ok { "ok" } else { "fail" }));

    if report.checks.is_empty() {
        lines.push("no checks registered".to_string());
        return lines.join("\n");
    }

    for check in &report.checks {
        lines.push(render_check_header(check));
        lines.push(format!("  {}", check.message));
        if let Some(fix) = &check.fix {
            lines.push(format!("  fix: {fix}"));
        }
    }

    lines.join("\n")
}

pub(crate) fn exit_code_for_report(report: &DoctorReport) -> i32 {
    if report.ok { 0 } else { 1 }
}

fn render_check_header(check: &DoctorCheck) -> String {
    format!(
        "[{}] {} ({})",
        status_label(check.status),
        check.id,
        check.target
    )
}

fn status_label(status: DoctorCheckStatus) -> &'static str {
    match status {
        DoctorCheckStatus::Pass => "pass",
        DoctorCheckStatus::Fail => "fail",
        DoctorCheckStatus::Warn => "warn",
        DoctorCheckStatus::Skip => "skip",
    }
}
