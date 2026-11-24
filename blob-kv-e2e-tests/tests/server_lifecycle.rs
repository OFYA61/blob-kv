mod common;

use std::{net::TcpStream, time::Duration};

use common::{TestClient, TestServer};
use predicates::prelude::*;

#[test]
fn server_starts_and_serves_requests() {
    let server = TestServer::start();
    let client = TestClient::new(server.address());

    assert_ne!(server.address().port(), 0);

    client.set("lifecycle", "running").assert_success();
    client
        .get("lifecycle")
        .assert_success()
        .assert_stdout(predicate::str::contains("running"));
}

#[test]
fn server_does_not_serve_requests_after_shutdown() {
    let mut server = TestServer::start();
    let address = server.address();
    let client = TestClient::new(address);

    client.set("before-shutdown", "value").assert_success();
    server.stop();

    let result = client.get("before-shutdown");
    result
        .assert_failure()
        .assert_code(1)
        .assert_stderr(predicate::str::contains("failed to connect"));

    assert!(
        TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_err(),
        "server still accepted a connection after shutdown"
    );
}

#[test]
fn server_can_restart_with_previous_data() {
    let mut server = TestServer::start();
    let client = TestClient::new(server.address());

    client.set("persisted", "yes").assert_success();

    server.restart();

    let client = TestClient::new(server.address());
    let result = client.get("persisted");
    result
        .assert_success()
        .assert_stdout(predicate::str::contains("yes"));
    assert_eq!(result.stdout(), "yes");
}

#[test]
fn a_second_server_cannot_acquire_the_locked_log_file() {
    let mut server = TestServer::start();
    let client = TestClient::new(server.address());

    client.set("locked", "yes").assert_success();

    let err = match TestServer::try_start_with_log_file(server.log_file()) {
        Ok(second) => panic!("second server unexpectedly started on {}", second.address()),
        Err(err) => err,
    };

    assert_eq!(
        err.status().and_then(|status| status.code()),
        Some(1),
        "unexpected second-server status: {err}"
    );
    assert!(
        err.stdout().contains("Failed to initalize engine"),
        "missing engine initialization error in server stdout: {}",
        err.stdout()
    );
    assert!(
        err.stdout().contains("Lock") || err.stdout().contains("WouldBlock"),
        "missing log lock error in server stdout: {}",
        err.stdout()
    );

    // The failed second server must not affect the running one.
    client
        .get("locked")
        .assert_success()
        .assert_stdout(predicate::str::contains("yes"));

    // Once the first server stops, the lock is released and the log can be
    // opened by a new server.
    server.stop();
    let reopened = TestServer::start_with_log_file(server.log_file());
    let client = TestClient::new(reopened.address());
    client
        .get("locked")
        .assert_success()
        .assert_stdout(predicate::str::contains("yes"));
}
