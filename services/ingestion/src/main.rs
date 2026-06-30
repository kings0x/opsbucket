use opsbucket_ingestion::config::Config;
use opsbucket_ingestion::server::Server;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&config.rust_log))
        .init();

    let server = Server::new(config);
    server.run().await
}
