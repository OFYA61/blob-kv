mod common;

use common::{TestClient, TestServer};
use predicates::prelude::*;

#[test]
fn sequence_of_operations_returns_most_recent_values() {
    let server = TestServer::start();
    let client = TestClient::new(server.address());

    client.set("alpha", "1").assert_success();
    client.set("beta", "2").assert_success();
    client.set("gamma", "3").assert_success();

    client
        .get("alpha")
        .assert_success()
        .assert_stdout(predicate::str::contains("1"));
    client
        .get("beta")
        .assert_success()
        .assert_stdout(predicate::str::contains("2"));
    client
        .get("gamma")
        .assert_success()
        .assert_stdout(predicate::str::contains("3"));

    // Overwriting a key must return the newest value.
    client.set("alpha", "10").assert_success();
    let overwritten = client.get("alpha");
    overwritten
        .assert_success()
        .assert_stdout(predicate::str::contains("10"));
    assert_eq!(overwritten.stdout(), "10");

    // Deleting one key leaves the others untouched.
    client.del("beta").assert_success();
    client
        .get("beta")
        .assert_success()
        .assert_stdout(predicate::str::is_empty())
        .assert_stderr(predicate::str::contains("error: KeyNotFound"));
    client
        .get("gamma")
        .assert_success()
        .assert_stdout(predicate::str::contains("3"));

    // A key can be recreated after deletion.
    client.set("beta", "20").assert_success();
    let recreated = client.get("beta");
    recreated
        .assert_success()
        .assert_stdout(predicate::str::contains("20"));
    assert_eq!(recreated.stdout(), "20");
}

#[test]
fn keys_and_values_with_spaces_and_unicode_round_trip() {
    let server = TestServer::start();
    let client = TestClient::new(server.address());

    client.set("hello world", "first value").assert_success();
    client
        .get("hello world")
        .assert_success()
        .assert_stdout(predicate::str::contains("first value"));

    client.set("clé-日本語-🔑", "héllo 🌍 — Ω").assert_success();
    client
        .get("clé-日本語-🔑")
        .assert_success()
        .assert_stdout(predicate::str::contains("héllo 🌍 — Ω"));

    // Overwrite the quoted key and verify the old value is gone.
    client.set("hello world", "second value").assert_success();
    let updated = client.get("hello world");
    updated
        .assert_success()
        .assert_stdout(predicate::str::contains("second value"));
    assert_eq!(updated.stdout(), "second value");
}
