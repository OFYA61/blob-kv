# blob-kv

A key-value store database backed by an append-only log.

## Crates

- **`blob-kv-engine`** — The engine behind the key-value store. Handles reading and writing data to the append-only log.
- **`blob-kv-server`** — The key-value store server. Exposes the engine over the network.
- **`blob-kv-cli`** — A binary application used to test and interact with the server.

More documentation will be added as the project grows.
