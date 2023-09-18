FROM rust:slim-bookworm as builder
WORKDIR /usr/src/bb
COPY . .
RUN cargo install --path .

FROM debian:bookworm-slim
COPY --from=builder /usr/local/cargo/bin/bb /usr/bin/bb
