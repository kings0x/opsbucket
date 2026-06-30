FROM rust:1.81-slim AS builder

WORKDIR /app
COPY . .

RUN cargo build -p opsbucket-migrator --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/opsbucket-migrator /usr/local/bin/opsbucket-migrator

CMD ["opsbucket-migrator", "up"]
