mod auth;
mod config;
mod routes;

use std::net::SocketAddr;

use clap::Parser;
use config::Settings;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "imagineration.toml")]
    config: std::path::PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let args = Args::parse();
    tracing::info!(config = %args.config.display(), "loading configuration");
    let settings = Settings::load(&args.config)?;
    let addr: SocketAddr = format!("{}:{}", settings.server.host, settings.server.port).parse()?;
    let app = routes::router(settings)?;

    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %error, "failed to install Ctrl+C handler");
        return;
    }
    tracing::info!("shutdown signal received, draining in-flight requests");
}
