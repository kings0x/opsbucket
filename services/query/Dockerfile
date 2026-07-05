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

RUN cargo build -p opsbucket-query --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsasl2-2 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/opsbucket-query /usr/local/bin/opsbucket-query

CMD ["opsbucket-query"]
