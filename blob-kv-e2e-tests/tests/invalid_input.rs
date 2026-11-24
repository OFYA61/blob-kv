mod common;

use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

use common::{TestClient, TestServer};
use predicates::prelude::*;

#[test]
fn malformed_commands_do_not_crash_the_cli() {
    let server = TestServer::start();
    let client = TestClient::new(server.address());

    let invalid_commands = [
        "",
        "GET",
        "SET",
        "DEL",
        "NOPE hello",
        "GET hello world",
        "SET key value extra",
        "GET \"unterminated",
        "GET 'unterminated",
        "SET \"\" value",
        "GET \"\"",
    ];

    for command in invalid_commands {
        let result = client.run(command);

        // Parse errors and server-side errors are reported, but the REPL keeps
        // running and exits normally at end of input.
        result
            .assert_success()
            .assert_stdout(predicate::str::is_empty())
            .assert_stderr(predicate::str::is_empty().not());
    }

    // The server and CLI must still work after all the invalid requests.
    client.set("still-works", "yes").assert_success();
    client
        .get("still-works")
        .assert_success()
        .assert_stdout(predicate::str::contains("yes"));
}

#[test]
fn invalid_cli_arguments_fail_cleanly() {
    let server = TestServer::start();
    let client = TestClient::new(server.address());

    let result = client.run_with_args(&["--not-a-real-option"]);
    result
        .assert_failure()
        .assert_code(2)
        .assert_stderr(predicate::str::contains("unexpected argument"));
}

#[test]
fn server_rejects_a_malformed_protocol_request() {
    let server = TestServer::start();
    let mut stream = TcpStream::connect(server.address()).expect("failed to connect to server");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("failed to set read timeout");

    // A 16-byte request header with an invalid protocol version and no payload.
    stream
        .write_all(&[0u8; 16])
        .expect("failed to write malformed request");

    let mut header = [0u8; 16];
    stream
        .read_exact(&mut header)
        .expect("failed to read invalid-request response");

    assert_eq!(header[0], 1, "unexpected response version");
    assert_eq!(header[1], 0x04, "expected the InvalidRequest status");

    // The server must remain usable after rejecting the malformed request.
    let client = TestClient::new(server.address());
    client
        .set("after-invalid-request", "still alive")
        .assert_success();
    client
        .get("after-invalid-request")
        .assert_success()
        .assert_stdout(predicate::str::contains("still alive"));
}
