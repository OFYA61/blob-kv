mod common;

use common::{TestClient, TestServer};
use predicates::prelude::*;

#[test]
fn values_survive_restart_with_same_log_file() {
    let mut server = TestServer::start();
    let log_file = server.log_file().to_path_buf();

    {
        let client = TestClient::new(server.address());

        client.set("alpha", "one").assert_success();
        client.set("beta", "two").assert_success();
        client.set("gamma", "three").assert_success();
        client.set("overwritten", "old").assert_success();
        client.set("overwritten", "new").assert_success();
        client.del("gamma").assert_success();
    }

    server.stop();
    assert!(log_file.exists(), "log file was removed unexpectedly");
    assert!(
        log_file.metadata().expect("log metadata").len() > 0,
        "log file is empty"
    );

    // Keep the first server (and therefore its temporary directory) alive
    // while the new server opens the same log file.
    let restarted = TestServer::start_with_log_file(&log_file);
    let client = TestClient::new(restarted.address());

    let alpha = client.get("alpha");
    alpha
        .assert_success()
        .assert_stdout(predicate::str::contains("one"));
    assert_eq!(alpha.stdout(), "one");

    let beta = client.get("beta");
    beta.assert_success()
        .assert_stdout(predicate::str::contains("two"));
    assert_eq!(beta.stdout(), "two");

    let overwritten = client.get("overwritten");
    overwritten
        .assert_success()
        .assert_stdout(predicate::str::contains("new"));
    assert_eq!(overwritten.stdout(), "new");

    let deleted = client.get("gamma");
    deleted
        .assert_success()
        .assert_stdout(predicate::str::is_empty())
        .assert_stderr(predicate::str::contains("error: KeyNotFound"));
}

#[test]
fn newly_written_values_survive_a_restart() {
    let mut server = TestServer::start();

    {
        let client = TestClient::new(server.address());
        client.set("first", "before").assert_success();
    }

    server.restart();

    {
        let client = TestClient::new(server.address());
        client
            .get("first")
            .assert_success()
            .assert_stdout(predicate::str::contains("before"));
        client.set("second", "after").assert_success();
    }

    server.restart();

    let client = TestClient::new(server.address());
    client
        .get("first")
        .assert_success()
        .assert_stdout(predicate::str::contains("before"));
    client
        .get("second")
        .assert_success()
        .assert_stdout(predicate::str::contains("after"));
}
