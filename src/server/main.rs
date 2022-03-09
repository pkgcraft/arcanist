use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::{Arg, Command};
use futures::TryFutureExt;
use pkgcraft::config::Config as PkgcraftConfig;
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::RwLock;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tracing_subscriber::{filter::LevelFilter, fmt};

use crate::service::ArcanistService;
use crate::settings::Settings;

mod service;
mod settings;
mod uds;

#[rustfmt::skip]
pub fn cmd() -> Command<'static> {
    Command::new(env!("CARGO_BIN_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("package-building daemon leveraging pkgcraft")
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
        .arg(Arg::new("socket")
            .takes_value(true)
            .forbid_empty_values(true)
            .long("bind")
            .value_name("IP:port")
            .help("bind to given network socket"))
        .arg(Arg::new("config")
            .takes_value(true)
            .forbid_empty_values(true)
            .long("config")
            .value_name("PATH")
            .help("path to config file"))
}

fn load_settings() -> Result<(Settings, PkgcraftConfig)> {
    let app = cmd();
    let args = app.get_matches();
    let config_file = args.value_of("config");
    let skip_config = config_file == Some("no");

    // load pkgcraft config
    let config = PkgcraftConfig::new("pkgcraft", "", !skip_config)
        .context("failed loading pkgcraft config")?;

    // load config settings and then override them with command-line settings
    let mut settings = Settings::new(&config, config_file, skip_config)?;

    if args.is_present("debug") {
        settings.debug = true;
    }
    settings.verbosity += args.occurrences_of("verbose") as i32;
    settings.verbosity -= args.occurrences_of("quiet") as i32;

    if let Some(socket) = args.value_of("socket") {
        settings.socket = socket.to_string();
    } else if settings.socket.is_empty() {
        // default to using unix domain socket
        settings.socket = config
            .path
            .run
            .join("arcanist.sock")
            .to_string_lossy()
            .into_owned();
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

    let subscriber = fmt().with_max_level(tracing_filter).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    Ok((settings, config))
}

#[tokio::main]
async fn main() -> Result<()> {
    let (settings, config) = load_settings()?;
    let socket = settings.socket.clone();
    let service = ArcanistService {
        settings,
        config: Arc::new(RwLock::new(config)),
    };
    let server = Server::builder().add_service(arcanist::Server::new(service));

    match socket.parse::<SocketAddr>() {
        // force unix domain sockets to be absolute paths
        Err(_) if socket.starts_with('/') => {
            uds::verify_socket_path(&socket)?;
            let listener = UnixListener::bind(&socket)
                .context(format!("failed binding to socket: {:?}", &socket))?;
            eprintln!("arcanist listening at: {}", &socket);
            let incoming = {
                async_stream::stream! {
                    loop {
                        let item = listener.accept().map_ok(|(st, _)| uds::UnixStream(st)).await;
                        yield item;
                    }
                }
            };
            server.serve_with_incoming(incoming).await?;
        }
        Ok(socket) => {
            let listener = TcpListener::bind(&socket)
                .await
                .context(format!("failed binding to socket: {:?}", &socket))?;
            let addr = listener
                .local_addr()
                .context(format!("invalid local address: {:?}", &socket))?;
            eprintln!("arcanist listening at: {}", &addr);
            let incoming = TcpListenerStream::new(listener);
            server.serve_with_incoming(incoming).await?
        }
        _ => bail!("invalid socket: {:?}", &socket),
    }

    Ok(())
}
