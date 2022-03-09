use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command};

use crate::Client;
use arcanist::proto::ListRequest;

#[rustfmt::skip]
pub fn cmd() -> Command<'static> {
    Command::new("sync")
        .about("sync repos")
        .arg(Arg::new("repos")
            .takes_value(true)
            .multiple_values(true)
            .value_name("REPO")
            .help("repos to sync"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let repos: Vec<String> = args
        .values_of("repos")
        .map(|names| names.map(|s| s.to_string()).collect())
        .unwrap_or_else(Vec::new);

    let request = tonic::Request::new(ListRequest { data: repos });
    client
        .sync_repos(request)
        .await
        .context("failed syncing repo(s)")?;

    Ok(())
}
