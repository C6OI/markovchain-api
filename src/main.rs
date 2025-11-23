#![warn(clippy::all, clippy::nursery, clippy::pedantic)]

mod client_ip;
mod database;
mod generator;
mod input;
mod migrations;
mod request_logging;
mod server;
mod settings;

use crate::database::create_pool;
use crate::migrations::Migrations;
use crate::settings::Settings;
use anyhow::Result;
use deadpool_postgres::Pool;
use std::env;
use std::io::stdout;
use std::path::Path;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[allow(unused)]
struct AppState {
    settings: Settings,
    pool: Pool,
}

#[tokio::main]
async fn main() -> Result<()> {
    #[rustfmt::skip]
    let env_filter = EnvFilter::builder().parse_lossy(
        env::var("RUST_LOG")
          .as_deref()
          .unwrap_or("info"),
    );

    let file_appender = tracing_appender::rolling::hourly("logs", "rolling.log");
    let (non_blocking_file, _file_guard) = tracing_appender::non_blocking(file_appender);
    let (non_blocking_stdout, _stdout_guard) = tracing_appender::non_blocking(stdout());
    let console = tracing_subscriber::fmt::layer().with_writer(non_blocking_stdout);

    let file = tracing_subscriber::fmt::layer()
        .json()
        .with_ansi(false)
        .with_writer(non_blocking_file);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(console)
        .with(file)
        .init();

    info!("Welcome to {}", env!("CARGO_PKG_NAME"));

    let settings = Settings::parse().unwrap_or_else(|err| panic!("Failed to load settings: {err}"));

    let pool = create_pool(&settings.database).await?;
    let client = pool.get().await?;

    Migrations::new("version_info".into(), Path::new("migrations"))?
        .up(&client)
        .await?;

    let state = Arc::new(AppState { settings, pool });
    server::start(state).await
}
