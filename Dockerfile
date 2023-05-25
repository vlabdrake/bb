FROM rust:slim-bookworm as builder
WORKDIR /usr/src/bb
COPY . .
RUN apt-get update && apt-get install -y pkg-config libssl-dev libgit2-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install --path .

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 libgit2-1.5 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/bb /usr/bin/bb
