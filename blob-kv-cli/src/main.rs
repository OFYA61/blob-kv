mod command;
mod config;

use blob_kv_client::{Client, Error as ClientError, Response};
use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, ReedlineError, Signal};

use crate::{command::Command, config::CliConfig};

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Reedline(#[from] ReedlineError),
}

fn print_response_error(response: &Response) {
    match response.payload.as_deref() {
        Some(payload) => eprintln!("error: {payload}"),
        None => eprintln!("error: {:?}", response.status),
    }
}

fn handle_get_response(response: &Response) {
    let status = response.status;

    match (status.is_ok(), response.payload.as_deref()) {
        (true, Some(payload)) => println!("{payload}"),
        (true, None) => println!(),
        (false, Some(payload)) => eprintln!("error: {payload}"),
        (false, None) => print_response_error(response),
    }
}

fn handle_write_response(response: &Response) {
    if !response.status.is_ok() {
        print_response_error(response);
    }
}

fn main() {
    let config = CliConfig::parse();

    if let Err(err) = run(config) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run(config: CliConfig) -> Result<(), CliError> {
    let mut client = Client::connect(&config.server_addr())?;

    let mut line_editor = Reedline::create();
    let prompt = DefaultPrompt::new(DefaultPromptSegment::Empty, DefaultPromptSegment::Empty);

    loop {
        match line_editor.read_line(&prompt)? {
            Signal::Success(input) => {
                let command = match Command::from(&input) {
                    Ok(command) => command,
                    Err(err) => {
                        err.iter().for_each(|e| eprintln!("{}", e));
                        continue;
                    }
                };

                let result = match command {
                    Command::Get { key } => client
                        .get(&key)
                        .map(|response| handle_get_response(&response)),
                    Command::Set { key, value } => client
                        .set(&key, value.as_deref().unwrap_or(""))
                        .map(|response| handle_write_response(&response)),
                    Command::Del { key } => client
                        .del(&key)
                        .map(|response| handle_write_response(&response)),
                };

                match result {
                    Ok(()) => {}
                    Err(ClientError::Request(err)) => eprintln!("error: {err}"),
                    Err(err) => return Err(err.into()),
                }
            }
            Signal::CtrlC(_) | Signal::CtrlD => {
                println!("\nAborted!");
                break;
            }
            signal => println!("Event: {signal:?}"),
        }
    }

    Ok(())
}
