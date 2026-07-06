FROM node:20-slim AS frontend-builder
WORKDIR /app/frontend
COPY services/dashboard/frontend/package.json services/dashboard/frontend/package-lock.json ./
RUN npm ci
COPY services/dashboard/frontend/ ./
RUN npm run build

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
COPY --from=frontend-builder /app/frontend/dist /app/services/dashboard/frontend/dist
RUN cargo build -p opsbucket-dashboard --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsasl2-2 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/opsbucket-dashboard /usr/local/bin/opsbucket-dashboard
CMD ["opsbucket-dashboard"]
