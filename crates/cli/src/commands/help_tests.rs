use clap::CommandFactory;

use super::Cli;

#[test]
fn root_help_mentions_subcommand_help_discovery() {
    let mut command = Cli::command();
    let help = command.render_help().to_string();

    assert!(help.contains("cueward <command> --help"));
}
