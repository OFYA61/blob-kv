mod config;
mod error;
mod server;

use blob_kv_engine::Engine;
use config::ServerConfig;

#[tokio::main]
async fn main() -> Result<(), ()> {
    let config = ServerConfig::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let engine = match Engine::init(config.log_file()) {
        Ok(engine) => engine,
        Err(err) => {
            tracing::error!("Failed to initalize engine: {:?}", err);
            return Err(());
        }
    };

    server::run(engine, config.host(), config.port()).await
}
