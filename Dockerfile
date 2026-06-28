FROM rust:1.83-slim-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN apt-get update && apt-get install -y pkg-config libavcodec-dev libavutil-dev libswscale-dev && \
    cargo build --release --features ffmpeg && \
    rm -rf /var/lib/apt/lists/*

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
        libavcodec59 \
        libavutil57 \
        libswscale6 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/chronodesk /usr/local/bin/
COPY --from=builder /app/target/release/signaling-server /usr/local/bin/

EXPOSE 21116

ENTRYPOINT ["signaling-server"]
CMD ["--bind", "0.0.0.0:21116"]
