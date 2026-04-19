use std::process;

use cueward_core::ShortcutAction;

use super::args::{ShortcutsAction, parse_reference};
use super::io::{load_actions_file, load_shortcut_spec};
use crate::commands::helpers::print_external;

pub(crate) fn dispatch(action: ShortcutsAction) {
    match action {
        ShortcutsAction::Create { name } => {
            let result = or_exit(cueward_adapter_macos::shortcuts::create_shortcut(&name));
            print_json("shortcuts/create", &result);
            eprintln!("shortcut created: {}", result.workflow_id);
        }
        ShortcutsAction::List => {
            let shortcuts = or_exit(cueward_adapter_macos::shortcuts::list_shortcuts_live());
            print_json("shortcuts/list", &shortcuts);
            eprintln!("{} shortcut(s)", shortcuts.len());
        }
        ShortcutsAction::Show { selector } => {
            let selector = selector.into_selector();
            let spec = or_exit(cueward_adapter_macos::shortcuts::export_shortcut_spec(&selector));
            print_json("shortcuts/show", &spec);
        }
        ShortcutsAction::Run { selector } => {
            let selector = selector.into_selector();
            or_exit(cueward_adapter_macos::shortcuts::run_shortcut(&selector));
            eprintln!("shortcut executed");
        }
        ShortcutsAction::Apply { path } => {
            let spec = or_exit(load_shortcut_spec(&path));
            let shortcut = or_exit(cueward_adapter_macos::shortcuts::apply_shortcut_spec(&spec));
            print_json("shortcuts/apply", &shortcut);
            eprintln!("shortcut updated: {}", shortcut.workflow_id);
        }
        ShortcutsAction::ValidateSpec { path } => {
            let spec = or_exit(load_shortcut_spec(&path));
            or_exit(cueward_adapter_macos::shortcuts::compile_actions(&spec));
            eprintln!("shortcut spec is valid");
        }
        ShortcutsAction::ExportSpec { selector } => {
            let selector = selector.into_selector();
            let spec = or_exit(cueward_adapter_macos::shortcuts::export_shortcut_spec(&selector));
            let yaml = or_exit(
                serde_yaml::to_string(&spec)
                    .map_err(|e| format!("failed to serialize shortcut spec: {e}")),
            );
            print_external("shortcuts/export-spec", &yaml);
        }
        ShortcutsAction::Rename { selector, new_name } => {
            let selector = selector.into_selector();
            let shortcut = or_exit(cueward_adapter_macos::shortcuts::rename_shortcut(&selector, &new_name));
            print_json("shortcuts/rename", &shortcut);
            eprintln!("shortcut renamed: {}", shortcut.workflow_id);
        }
        ShortcutsAction::Move { selector, folder } => {
            let selector = selector.into_selector();
            let shortcut = or_exit(cueward_adapter_macos::shortcuts::move_shortcut(&selector, &folder));
            print_json("shortcuts/move", &shortcut);
            eprintln!("shortcut moved: {}", shortcut.workflow_id);
        }
        ShortcutsAction::Surface { selector, surface } => {
            let selector = selector.into_selector();
            let shortcut =
                or_exit(cueward_adapter_macos::shortcuts::attach_surface(&selector, &surface.into()));
            print_json("shortcuts/surface", &shortcut);
            eprintln!("shortcut surface updated: {}", shortcut.workflow_id);
        }
        ShortcutsAction::InputType { selector, input_type } => {
            let selector = selector.into_selector();
            let shortcut =
                or_exit(cueward_adapter_macos::shortcuts::set_input_type(&selector, &input_type.into()));
            print_json("shortcuts/input-type", &shortcut);
            eprintln!("shortcut input type updated: {}", shortcut.workflow_id);
        }
        ShortcutsAction::AddText { selector, value, output } => {
            let selector = selector.into_selector();
            append_action(
                selector,
                ShortcutAction::Text { value, output },
                "shortcuts/add-text",
            );
        }
        ShortcutsAction::AddGetText { selector, from, output } => {
            let selector = selector.into_selector();
            append_action(
                selector,
                ShortcutAction::GetText {
                    from: parse_reference(&from),
                    output,
                },
                "shortcuts/add-get-text",
            );
        }
        ShortcutsAction::AddGetUrls { selector, from, output } => {
            let selector = selector.into_selector();
            append_action(
                selector,
                ShortcutAction::GetUrls {
                    from: parse_reference(&from),
                    output,
                },
                "shortcuts/add-get-urls",
            );
        }
        ShortcutsAction::AddReplaceText {
            selector,
            from,
            find,
            replace,
            regex,
            ignore_case,
            output,
        } => {
            let selector = selector.into_selector();
            append_action(
                selector,
                ShortcutAction::ReplaceText {
                    from: parse_reference(&from),
                    find,
                    replace,
                    regex,
                    ignore_case,
                    output,
                },
                "shortcuts/add-replace-text",
            );
        }
        ShortcutsAction::AddCopyToClipboard { selector, from } => {
            let selector = selector.into_selector();
            append_action(
                selector,
                ShortcutAction::CopyToClipboard {
                    from: parse_reference(&from),
                },
                "shortcuts/add-copy-to-clipboard",
            );
        }
        ShortcutsAction::AddShare { selector, from } => {
            let selector = selector.into_selector();
            append_action(
                selector,
                ShortcutAction::Share {
                    from: parse_reference(&from),
                },
                "shortcuts/add-share",
            );
        }
        ShortcutsAction::AddIf {
            selector,
            input,
            value,
            then_actions,
        } => {
            let selector = selector.into_selector();
            let then_actions = or_exit(load_actions_file(&then_actions));
            append_action(
                selector,
                ShortcutAction::IfEqualsText {
                    input: parse_reference(&input),
                    value,
                    then_actions,
                },
                "shortcuts/add-if",
            );
        }
        ShortcutsAction::AddRepeat {
            selector,
            input,
            body_actions,
        } => {
            let selector = selector.into_selector();
            let body = or_exit(load_actions_file(&body_actions));
            append_action(
                selector,
                ShortcutAction::RepeatEach {
                    input: parse_reference(&input),
                    body,
                },
                "shortcuts/add-repeat",
            );
        }
    }
}

fn append_action(
    selector: cueward_adapter_macos::shortcuts::ShortcutSelector,
    action: ShortcutAction,
    source: &str,
) {
    let shortcut = or_exit(cueward_adapter_macos::shortcuts::append_shortcut_action(
        &selector, &action,
    ));
    print_json(source, &shortcut);
    eprintln!("shortcut updated: {}", shortcut.workflow_id);
}

fn print_json<T: serde::Serialize>(source: &str, value: &T) {
    let json = or_exit(
        serde_json::to_string_pretty(value).map_err(|e| format!("failed to serialize output: {e}")),
    );
    print_external(source, &json);
}

fn or_exit<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(1);
        }
    }
}
