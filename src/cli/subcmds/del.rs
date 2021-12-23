use anyhow::Result;
use clap::{App, AppSettings, Arg, ArgMatches, ArgSettings};

use crate::Client;
use arcanist::proto::ListRequest;

#[rustfmt::skip]
pub fn cmd() -> App<'static> {
    App::new("del")
        .about("remove packages")
        .setting(AppSettings::DisableHelpSubcommand)
        .arg(Arg::new("pkgs")
            .setting(ArgSettings::TakesValue)
            .setting(ArgSettings::MultipleValues)
            .setting(ArgSettings::Required)
            .value_name("PKG")
            .help("packages to remove"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let pkgs: Vec<String> = args
        .values_of("pkgs")
        .unwrap()
        .map(|s| s.to_string())
        .collect();
    let request = tonic::Request::new(ListRequest { data: pkgs });
    let response = client.remove_packages(request).await?;
    let mut stream = response.into_inner();
    while let Some(response) = stream.message().await? {
        println!("{}", response.data);
    }
    Ok(())
}
