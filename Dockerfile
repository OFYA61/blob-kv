# syntax=docker/dockerfile:1

FROM rust:bookworm AS builder

WORKDIR /app

COPY . .

RUN cargo build --locked --release -p blob-kv-server -p blob-kv-cli

FROM debian:bookworm-slim

WORKDIR /app

COPY --from=builder /app/target/release/blob-kv-server /usr/local/bin/blob-kv-server
COPY --from=builder /app/target/release/blob-kv-cli /usr/local/bin/blob-kv-cli

RUN mkdir -p /data

EXPOSE 6379

ENTRYPOINT ["blob-kv-server"]
