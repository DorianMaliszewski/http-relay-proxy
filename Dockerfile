FROM rust as builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && apt-get clean


COPY ./ /app
RUN cargo build --release


FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y openssl ca-certificates && apt-get clean
WORKDIR /

COPY --from=builder /app/target/release/http-replay-proxy /http-replay-proxy

VOLUME ["/records"]
EXPOSE 3333

ENTRYPOINT ["/http-replay-proxy"]
