mod common;

use common::{TestClient, TestServer};
use predicates::prelude::*;

#[test]
fn getting_a_missing_key_reports_key_not_found() {
    let server = TestServer::start();
    let client = TestClient::new(server.address());

    let result = client.get("does-not-exist");
    result
        .assert_success()
        .assert_stdout(predicate::str::is_empty())
        .assert_stderr(predicate::str::contains("error: KeyNotFound"));
}

#[test]
fn deleting_a_missing_key_reports_key_not_found() {
    let server = TestServer::start();
    let client = TestClient::new(server.address());

    let first_delete = client.del("does-not-exist");
    first_delete
        .assert_success()
        .assert_stdout(predicate::str::is_empty())
        .assert_stderr(predicate::str::contains("error: KeyNotFound"));

    // Deletion of a missing key is idempotent and does not crash the CLI.
    let second_delete = client.del("does-not-exist");
    second_delete
        .assert_success()
        .assert_stdout(predicate::str::is_empty())
        .assert_stderr(predicate::str::contains("error: KeyNotFound"));

    client
        .get("does-not-exist")
        .assert_success()
        .assert_stderr(predicate::str::contains("error: KeyNotFound"));
}

#[test]
fn deleting_an_existing_key_twice_reports_missing_on_second_delete() {
    let server = TestServer::start();
    let client = TestClient::new(server.address());

    client.set("temporary", "value").assert_success();

    let first_delete = client.del("temporary");
    first_delete
        .assert_success()
        .assert_stdout(predicate::str::is_empty())
        .assert_stderr(predicate::str::is_empty());

    let second_delete = client.del("temporary");
    second_delete
        .assert_success()
        .assert_stdout(predicate::str::is_empty())
        .assert_stderr(predicate::str::contains("error: KeyNotFound"));

    client
        .get("temporary")
        .assert_success()
        .assert_stdout(predicate::str::is_empty())
        .assert_stderr(predicate::str::contains("error: KeyNotFound"));
}
