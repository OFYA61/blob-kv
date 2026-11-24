//! End-to-end tests for the `blob-kv` server and CLI.
//!
//! The reusable test harness lives in `tests/common`, because it relies on the
//! `CARGO_BIN_EXE_*` environment variables that Cargo provides to integration
//! tests. This library target only exists so the package has a regular
//! (non-test) target.
