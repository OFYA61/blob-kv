
Build a small, Redis-inspired database. An append-only binary log, record codec, hash index. With the ability to replay records, rebuild index, and handle incomplete writes.

Language: Rust
Platform: Linux
Data model: UTF-8 string keys and values
Operations: `SET`, `GET`, `DEL`
Index: an in-memory `HashMap`
Persistence: an append-only binary log
Server: a simple line-oriented TCP protocol
Concurrency: one writer, with a simple synchronzation model