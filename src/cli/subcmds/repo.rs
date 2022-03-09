use anyhow::Result;
use clap::{ArgMatches, Command};

use crate::settings::Settings;
use crate::Client;

mod add;
mod del;
mod list;
mod new;
mod sync;

#[rustfmt::skip]
pub fn cmd() -> Command<'static> {
    Command::new("repo")
        .about("manage repos")
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(add::cmd())
        .subcommand(del::cmd())
        .subcommand(list::cmd())
        .subcommand(new::cmd())
        .subcommand(sync::cmd())
}

pub async fn run(args: &ArgMatches, client: &mut Client, _settings: &mut Settings) -> Result<()> {
    let (subcmd, m) = args.subcommand().unwrap();
    match subcmd {
        "add" => add::run(m, client).await,
        "del" => del::run(m, client).await,
        "list" => list::run(client).await,
        "new" => new::run(m, client).await,
        "sync" => sync::run(m, client).await,
        _ => panic!("unknown subcommand"),
    }
}
