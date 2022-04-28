use std::io;
use std::net::SocketAddr;
use std::process;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Arg, ArgMatches, Command};
use pkgcraft::config::Config as PkgcraftConfig;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;
use tracing_subscriber::{filter::LevelFilter, fmt};
use url::Url;

use argparse::{positive_int, str_to_bool};
use settings::Settings;

mod argparse;
mod settings;
mod subcmds;

pub type Client = arcanist::Client<Channel>;

#[rustfmt::skip]
pub fn cmd() -> Command<'static> {
    Command::new(env!("CARGO_BIN_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("command-line tool leveraging pkgcraft")
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommands(subcmds::register())
        .arg(Arg::new("color")
            .takes_value(true)
            .forbid_empty_values(true)
            .long("color")
            .value_name("BOOLEAN")
            .validator(str_to_bool)
            .help("toggle colored output"))
        .arg(Arg::new("debug")
            .long("debug")
            .help("enable debug output"))
        .arg(Arg::new("verbose")
            .multiple_occurrences(true)
            .short('v')
            .long("verbose")
            .help("enable verbose output"))
        .arg(Arg::new("quiet")
            .multiple_occurrences(true)
            .short('q')
            .long("quiet")
            .help("suppress non-error messages"))
        .arg(Arg::new("config")
            .takes_value(true)
            .forbid_empty_values(true)
            .long("config")
            .value_name("PATH")
            .help("path to config file"))
        .arg(Arg::new("config-none")
            .long("config-none")
            .help("don't load config file"))
        .arg(Arg::new("url")
            .takes_value(true)
            .forbid_empty_values(true)
            .short('c')
            .long("connect")
            .value_name("URL")
            .help("connect to given arcanist instance"))
        .arg(Arg::new("timeout")
            .takes_value(true)
            .forbid_empty_values(true)
            .long("timeout")
            .value_name("SECONDS")
            .default_value("5")
            .validator(positive_int)
            .help("connection timeout"))
}

fn load_settings() -> Result<(Settings, PkgcraftConfig, ArgMatches)> {
    let app = cmd();
    let args = app.get_matches();
    let config_file = args.value_of("config");
    let skip_config = args.is_present("config-none");

    // load pkgcraft config
    let config = PkgcraftConfig::new("pkgcraft", "", !skip_config)
        .context("failed loading pkgcraft config")?;

    // load config settings and then override them with command-line settings
    let mut settings = Settings::new(&config, config_file, skip_config)?;

    if let Some(color) = args.value_of("color") {
        settings.color = str_to_bool(color)?;
    }

    if args.is_present("debug") {
        settings.debug = true;
    }
    settings.verbosity += args.occurrences_of("verbose") as i32;
    settings.verbosity -= args.occurrences_of("quiet") as i32;

    if let Some(url) = args.value_of("url") {
        // convert raw socket arg into url
        settings.url = match url.parse::<SocketAddr>() {
            Err(_) => url.to_string(),
            Ok(socket) => format!("http://{socket}"),
        };
    }

    // defaults to warning level
    let tracing_filter = match settings.verbosity {
        i32::MIN..=-2 => LevelFilter::OFF,
        -1 => LevelFilter::ERROR,
        0 => LevelFilter::WARN,
        1 => LevelFilter::INFO,
        2 => LevelFilter::DEBUG,
        3..=i32::MAX => LevelFilter::TRACE,
    };

    let subscriber = fmt()
        .with_max_level(tracing_filter)
        .with_writer(io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    Ok((settings, config, args))
}

#[tokio::main]
async fn try_main() -> Result<()> {
    let (settings, config, args) = load_settings()?;
    let user_agent = format!("{}-{}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
    let timeout = args
        .value_of("timeout")
        .unwrap_or_default()
        .parse::<u64>()
        .unwrap();

    // use unix domain socket by default if no connection URL is given
    let url = match settings.url.is_empty() {
        false => settings.url.clone(),
        true => {
            let path = config.path.run.join("arcanist.sock");
            arcanist::connect_or_spawn(&path, Some(timeout)).await?
        }
    };

    // connect to arcanist
    let channel: Channel = match Url::parse(&url) {
        Err(_) => {
            let error = format!("failed connecting to arcanist socket: {url}");
            Endpoint::from_static("http://[::]")
                .user_agent(user_agent)?
                .connect_with_connector(service_fn(move |_: Uri| UnixStream::connect(url.clone())))
                .await
                .context(error)?
        }
        Ok(_) => {
            let error = format!("failed connecting to arcanist: {url}");
            Endpoint::from_shared(url)?
                .connect_timeout(Duration::from_secs(timeout))
                .user_agent(user_agent)?
                .connect()
                .await
                .context(error)?
        }
    };

    let mut client: Client = arcanist::Client::new(channel);
    subcmds::run(&args, &mut client, &settings).await
}

fn main() {
    // extract error message from tonic status responses
    if let Err(error) = try_main() {
        eprintln!("error: {error}\n");
        error
            .chain()
            .skip(1)
            .for_each(|cause| match cause.downcast_ref() {
                Some(e @ tonic::Status { .. }) => eprintln!("caused by: {}", e.message()),
                _ => eprintln!("caused by: {cause}"),
            });
        process::exit(1);
    }
}
