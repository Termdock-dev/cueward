mod commands;

use clap::Parser;

fn main() {
    let cli = commands::Cli::parse();
    commands::dispatch(cli.command);
}
