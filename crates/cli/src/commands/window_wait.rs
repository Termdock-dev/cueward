use clap::Args;
use cueward_adapter_macos::window::{WaitCondition, WaitOptions, WaitSelector, wait_for_window};

#[derive(Args)]
pub(crate) struct WindowWaitArgs {
    /// input_target from a recent snapshot; waiting sends no events.
    #[arg(long)]
    target: String,
    #[arg(long, value_parser = ["element-exists", "element-absent", "value-equals", "enabled", "window-gone"])]
    condition: String,
    /// AX role required for element conditions.
    #[arg(long)]
    role: Option<String>,
    /// Exact accessible name, as observed in window inspect.
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    identifier: Option<String>,
    /// Exact expected value, only for value-equals.
    #[arg(long, allow_hyphen_values = true)]
    value: Option<String>,
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 100)]
    interval_ms: u64,
}

pub(crate) fn dispatch(args: WindowWaitArgs) {
    let condition = match args.condition.as_str() {
        "element-exists" => WaitCondition::ElementExists,
        "element-absent" => WaitCondition::ElementAbsent,
        "value-equals" => WaitCondition::ValueEquals,
        "enabled" => WaitCondition::Enabled,
        _ => WaitCondition::WindowGone,
    };
    let selector = if args.role.is_some() || args.name.is_some() || args.identifier.is_some() {
        Some(WaitSelector {
            role: args.role.unwrap_or_default(),
            name: args.name,
            identifier: args.identifier,
        })
    } else {
        None
    };
    let options = WaitOptions {
        condition,
        selector,
        value: args.value,
        timeout_ms: args.timeout_ms,
        interval_ms: args.interval_ms,
    };
    super::window::output("window/wait", wait_for_window(&args.target, &options));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{Cli, Command, WindowAction};
    use clap::Parser;
    #[test]
    fn parses_wait_conditions_and_exact_selectors() {
        for condition in [
            "element-exists",
            "element-absent",
            "value-equals",
            "enabled",
            "window-gone",
        ] {
            let parsed = Cli::try_parse_from([
                "cueward",
                "window",
                "wait",
                "--target",
                "token",
                "--condition",
                condition,
            ])
            .expect("wait");
            assert!(matches!(
                parsed.command,
                Command::Window {
                    action: WindowAction::Wait { .. }
                }
            ));
        }
        let parsed = Cli::try_parse_from([
            "cueward",
            "window",
            "wait",
            "--target",
            "token",
            "--condition",
            "value-equals",
            "--role",
            "AXTextField",
            "--name",
            "Title",
            "--value",
            "",
            "--timeout-ms",
            "2500",
        ])
        .expect("value wait");
        assert!(
            matches!(parsed.command, Command::Window { action: WindowAction::Wait { args: WindowWaitArgs { value: Some(ref v), timeout_ms: 2500, .. } } } if v.is_empty())
        );
        assert!(
            Cli::try_parse_from(["cueward", "window", "wait", "--condition", "enabled"]).is_err()
        );
    }
}
