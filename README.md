# blob-kv

[![Build and Test](https://github.com/OFYA61/blob-kv/actions/workflows/build-and-test.yml/badge.svg)](https://github.com/OFYA61/blob-kv/actions/workflows/build-and-test.yml)
[![Release](https://img.shields.io/github/v/release/OFYA61/blob-kv)](https://github.com/OFYA61/blob-kv/releases/latest)

## Contents

- [Description](#description)
- [Project structure](#project-structure)
- [Quick start](#quick-start)
  - [With Docker](#with-docker)
  - [Local](#local)
  - [Example commands](#example-commands)
- [Configuration](#configuration)
  - [`blob-kv-cli`](#blob-kv-cli)
  - [`blob-kv-server`](#blob-kv-server)
- [Running tests](#running-tests)
- [Limitations](#limitations)

## Description

`blob-kv` is a lightweight key-value database backed by an append-only log. It provides a TCP server, and a CLI client for interacting with the database.
The project is built to explore database internals, persistent storage, network protocols, and systems programming.

A key-value store database backed by an append-only log.

## Project structure

The following is the list of subcrates:
- **`blob-kv-cli`** - A binary application used to test and interact with the server.
- **`blob-kv-client`** - A client library for connecting applications to the server.
- **`blob-kv-e2e-tests`** - End-to-end tests that run the real server and CLI through a shared test harness.
- **`blob-kv-engine`** - The engine behind the key-value store. Handles reading and writing data to the append-only log.
- **`blob-kv-protocol`** - Network protocol.
- **`blob-kv-server`** - The key-value store server. Exposes the engine over the network.

## Quick start

### With Docker

Build and start the server in the background:

```bash
docker compose up --build -d
```

Open the interactive CLI inside the running container:

```bash
docker compose exec blob-kv blob-kv-cli
```

The database is stored in the named volume `blob-kv-data`, so it survives container restarts.

Stop the container while keeping the database:

```bash
docker compose down
```

Stop the container and delete the database volume:

```bash
docker compose down -v
```

### Local

```bash
# Start the server
cargo run --release -p blob-kv-server

# Start the CLI client
cargo run --release -p blob-kv-cli
```

### Example commands

```text
SET hello world
GET hello
DEL hello
SET "key with whitespaces" 'value with whitespaces'
```

## Configuration

### `blob-kv-cli`

| Argument | Environment variable | Description | Default |
| --- | --- | --- | --- |
| `--host` | `BLOB_KV_CLI_HOST` | Server host to connect to | `127.0.0.1` |
| `--port` | `BLOB_KV_CLI_PORT` | Server port to connect to | `9999` |

### `blob-kv-server`

| Argument | Environment variable | Description | Default |
| --- | --- | --- | --- |
| `--host` | `BLOB_KV_SERVER_HOST` | Address to bind the server to | `0.0.0.0` |
| `--port` | `BLOB_KV_SERVER_PORT` | Port to listen on | `9999` |
| `--log-file` | `BLOB_KV_SERVER_LOG_FILE` | Path to the append-only log file | `test.log` |

Command-line arguments take precedence over environment variables.

## Running tests

```bash
cargo test --release --workspace
```

## Limitations

This is a side project that is being built for learning purposes, thus the following features are missing for the time being:
* no authentication
* no encryption/TLS
* no replication
* no automatic log compaction
* no concurrent write support
