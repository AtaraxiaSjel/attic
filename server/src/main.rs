use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use tokio::join;
use tokio::task::spawn;
use tracing_error::ErrorLayer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use attic_server::config;

/// Nix binary cache server.
#[derive(Debug, Parser)]
#[clap(version, author = "Zhaofeng Li <hello@zhaofeng.li>")]
#[clap(propagate_version = true)]
struct Opts {
    /// Path to the config file.
    #[clap(short = 'f', long)]
    config: Option<PathBuf>,

    /// Socket address to listen on.
    ///
    /// This overrides `listen` in the config.
    #[clap(short = 'l', long)]
    listen: Option<SocketAddr>,

    /// Mode to run.
    #[clap(long, default_value = "monolithic")]
    mode: ServerMode,

    /// Whether to enable tokio-console.
    ///
    /// The console server will listen on its default port.
    #[clap(long)]
    tokio_console: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ServerMode {
    /// Run all components.
    Monolithic,

    /// Run the API server.
    ApiServer,

    /// Run the garbage collector periodically.
    GarbageCollector,

    /// Run the database migrations then exit.
    DbMigrations,

    /// Run garbage collection then exit.
    GarbageCollectorOnce,

    /// Check the configuration then exit.
    CheckConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();

    init_logging(opts.tokio_console);
    dump_version();

    let config = if let Some(config_path) = opts.config {
        config::load_config_from_path(&config_path)
    } else if let Ok(config_env) = env::var("ATTIC_SERVER_CONFIG_BASE64") {
        let decoded = String::from_utf8(base64::decode(config_env.as_bytes())?)?;
        config::load_config_from_str(&decoded)
    } else {
        // Config from XDG
        let config_path = config::get_xdg_config_path()?;

        if opts.mode == ServerMode::Monolithic {
            // Special OOBE sequence
            attic_server::oobe::run_oobe().await?;
        } else if !config_path.exists() {
            eprintln!("You haven't specified a config file (--config/-f), and the XDG config file doesn't exist.");
            eprintln!("Hint: To automatically set up Attic, run `atticd` without any arguments.");
        }

        config::load_config_from_path(&config_path)
    };

    match opts.mode {
        ServerMode::Monolithic => {
            attic_server::run_migrations(config.clone()).await?;

            let (api_server, _) = join!(
                attic_server::run_api_server(opts.listen, config.clone()),
                attic_server::gc::run_garbage_collection(config.clone()),
            );

            api_server?;
        }
        ServerMode::ApiServer => {
            attic_server::run_api_server(opts.listen, config).await?;
        }
        ServerMode::GarbageCollector => {
            attic_server::gc::run_garbage_collection(config.clone()).await;
        }
        ServerMode::DbMigrations => {
            attic_server::run_migrations(config).await?;
        }
        ServerMode::GarbageCollectorOnce => {
            attic_server::gc::run_garbage_collection_once(config).await?;
        }
        ServerMode::CheckConfig => {
            // config is valid, let's just exit :)
        }
    }

    Ok(())
}

fn init_logging(tokio_console: bool) {
    let env_filter = EnvFilter::from_default_env();
    let fmt_layer = tracing_subscriber::fmt::layer().with_filter(env_filter);

    let error_layer = ErrorLayer::default();

    let console_layer = if tokio_console {
        let (layer, server) = console_subscriber::ConsoleLayer::new();
        spawn(server.serve());
        Some(layer)
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(error_layer)
        .with(console_layer)
        .init();

    if tokio_console {
        eprintln!("Note: tokio-console is enabled");
    }
}

fn dump_version() {
    #[cfg(debug_assertions)]
    eprintln!("Attic Server {} (debug)", env!("CARGO_PKG_VERSION"));

    #[cfg(not(debug_assertions))]
    eprintln!("Attic Server {} (release)", env!("CARGO_PKG_VERSION"));
}
