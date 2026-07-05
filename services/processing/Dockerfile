FROM rust:stable-slim AS builder

RUN apt-get update && apt-get install -y \
    build-essential \
    ca-certificates \
    cmake \
    libssl-dev \
    libsasl2-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . ./

RUN cargo build -p opsbucket-processing --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsasl2-2 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/opsbucket-processing /usr/local/bin/opsbucket-processing

CMD ["opsbucket-processing"]
