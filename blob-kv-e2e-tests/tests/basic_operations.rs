mod common;

use common::{TestClient, TestServer};
use predicates::prelude::*;

#[test]
fn set_get_and_delete_round_trip() {
    let server = TestServer::start();
    let client = TestClient::new(server.address());

    let set = client.set("username", "alice");
    set.assert_success()
        .assert_stdout(predicate::str::is_empty());

    let get = client.get("username");
    get.assert_success()
        .assert_stdout(predicate::str::contains("alice"));
    assert_eq!(get.stdout(), "alice");

    let deleted = client.del("username");
    deleted
        .assert_success()
        .assert_stdout(predicate::str::is_empty());

    let missing = client.get("username");
    missing
        .assert_success()
        .assert_stdout(predicate::str::is_empty())
        .assert_stderr(predicate::str::contains("error: KeyNotFound"));
}

#[test]
fn empty_values_are_distinct_from_missing_keys() {
    let server = TestServer::start();
    let client = TestClient::new(server.address());

    client.set("empty", "").assert_success();

    let get = client.get("empty");
    get.assert_success()
        .assert_stdout(predicate::str::is_empty());
    assert_eq!(get.stdout(), "");

    client.del("empty").assert_success();

    client
        .get("empty")
        .assert_success()
        .assert_stderr(predicate::str::contains("error: KeyNotFound"));
}
