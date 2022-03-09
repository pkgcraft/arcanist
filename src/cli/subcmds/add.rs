use anyhow::Result;
use clap::{Arg, ArgMatches, Command};

use crate::Client;
use arcanist::proto::ListRequest;

#[rustfmt::skip]
pub fn cmd() -> Command<'static> {
    Command::new("add")
        .about("add packages")
        .disable_help_subcommand(true)
        .arg(Arg::new("pkgs")
            .required(true)
            .takes_value(true)
            .multiple_values(true)
            .value_name("PKG")
            .help("packages to install"))
}

pub async fn run(args: &ArgMatches, client: &mut Client) -> Result<()> {
    let pkgs: Vec<String> = args
        .values_of("pkgs")
        .unwrap()
        .map(|s| s.to_string())
        .collect();
    let request = tonic::Request::new(ListRequest { data: pkgs });
    let response = client.add_packages(request).await?;
    let mut stream = response.into_inner();
    while let Some(response) = stream.message().await? {
        println!("{}", response.data);
    }
    Ok(())
}
