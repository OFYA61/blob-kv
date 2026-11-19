use blob_kv_engine::Engine;

mod error;
mod server;

#[tokio::main]
async fn main() -> Result<(), ()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let engine = match Engine::init("test.log") {
        Ok(engine) => engine,
        Err(err) => {
            tracing::error!("Failed to initalize engine: {:?}", err);
            return Err(());
        }
    };

    server::run(engine).await
}
