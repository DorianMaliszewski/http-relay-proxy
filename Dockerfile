FROM rust:alpine3.16

WORKDIR /app

RUN apk add --no-cache musl-dev

RUN cargo install cargo-watch

COPY ./ ./
