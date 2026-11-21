use blob_kv_client::{Client, Error as ClientError, Response};
use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, ReedlineError, Signal};

const SERVER_ADDR: &str = "127.0.0.1:9999";

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Reedline(#[from] ReedlineError),
}

enum Command {
    Get(String),
    Set(String, String),
    Del(String),
}

fn parse_command(input: &str) -> Option<Command> {
    let mut parts = input.split(' ');
    let command = parts.next().unwrap_or_default();

    match command {
        "GET" => {
            let key = parts.next();
            if parts.next().is_some() {
                eprintln!("error: too many arguments for {command}");
                return None;
            }

            let Some(key) = key else {
                eprintln!("error: {command} expects a key");
                return None;
            };

            Some(Command::Get(key.to_owned()))
        }
        "SET" => {
            let key = parts.next();
            let value = parts.next().unwrap_or_default();
            if parts.next().is_some() {
                eprintln!("error: too many arguments for {command}");
                return None;
            }

            let Some(key) = key else {
                eprintln!("error: {command} expects a key");
                return None;
            };

            Some(Command::Set(key.to_owned(), value.to_owned()))
        }
        "DEL" => {
            let key = parts.next();
            if parts.next().is_some() {
                eprintln!("error: too many arguments for {command}");
                return None;
            }

            let Some(key) = key else {
                eprintln!("error: {command} expects a key");
                return None;
            };

            Some(Command::Del(key.to_owned()))
        }
        _ => {
            eprintln!("error: unknown command '{command}'");
            None
        }
    }
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
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let mut client = Client::connect(SERVER_ADDR)?;

    let mut line_editor = Reedline::create();
    let prompt = DefaultPrompt::new(DefaultPromptSegment::Empty, DefaultPromptSegment::Empty);

    loop {
        match line_editor.read_line(&prompt)? {
            Signal::Success(input) => {
                let Some(command) = parse_command(&input) else {
                    continue;
                };

                let result = match command {
                    Command::Get(key) => client
                        .get(&key)
                        .map(|response| handle_get_response(&response)),
                    Command::Set(key, value) => client
                        .set(&key, &value)
                        .map(|response| handle_write_response(&response)),
                    Command::Del(key) => client
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
