FROM rust:slim-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN apt-get update && apt-get install -y pkg-config libavcodec-dev libavutil-dev libavformat-dev libavdevice-dev libavfilter-dev libswresample-dev libswscale-dev libpostproc-dev libgbm-dev libxdo-dev libwayland-dev libxkbcommon-dev libegl1-mesa-dev libpipewire-0.3-dev libclang-dev libssl-dev && \
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

COPY --from=builder /app/target/release/chronodesk-bin /usr/local/bin/chronodesk
COPY --from=builder /app/target/release/signaling-server /usr/local/bin/

EXPOSE 21116

ENTRYPOINT ["signaling-server"]
CMD ["--bind", "0.0.0.0:21116"]
