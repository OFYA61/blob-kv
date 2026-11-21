use std::path::{Path, PathBuf};

use clap::Parser;

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 9999;
const DEFAULT_LOG_FILE: &str = "test.log";

#[derive(Debug, Parser)]
pub struct ServerConfig {
    #[arg(long, env = "BLOB_KV_SERVER_HOST")]
    host: Option<String>,

    #[arg(long, env = "BLOB_KV_SERVER_PORT")]
    port: Option<u16>,

    #[arg(long, env = "BLOB_KV_SERVER_LOG_FILE")]
    log_file: Option<PathBuf>,
}

impl ServerConfig {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    pub fn host(&self) -> &str {
        self.host.as_deref().unwrap_or(DEFAULT_HOST)
    }

    pub fn port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }

    pub fn log_file(&self) -> &Path {
        self.log_file
            .as_deref()
            .unwrap_or_else(|| Path::new(DEFAULT_LOG_FILE))
    }
}
