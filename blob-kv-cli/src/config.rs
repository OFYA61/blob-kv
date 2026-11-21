use clap::Parser;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 9999;

#[derive(Debug, Parser)]
pub struct CliConfig {
    #[arg(long, env = "BLOB_KV_CLI_HOST")]
    host: Option<String>,

    #[arg(long, env = "BLOB_KV_CLI_PORT")]
    port: Option<u16>,
}

impl CliConfig {
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    pub fn host(&self) -> &str {
        self.host.as_deref().unwrap_or(DEFAULT_HOST)
    }

    pub fn port(&self) -> u16 {
        self.port.unwrap_or(DEFAULT_PORT)
    }

    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.host(), self.port())
    }
}
