use crate::config::OpsBucketConfig;

pub fn generate_docker_compose(cfg: &OpsBucketConfig) -> String {
    format!(
        r#"services:
  caddy:
    container_name: opsbucket-caddy
    image: caddy:2-alpine
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ${{PWD}}/Caddyfile:/etc/caddy/Caddyfile
      - caddy_data:/data
      - caddy_config:/config
    depends_on:
      dashboard:
        condition: service_started
      ingestion:
        condition: service_started
      query:
        condition: service_started
    restart: unless-stopped
    networks:
      - opsbucket

  redpanda:
    container_name: opsbucket-redpanda
    image: docker.redpanda.com/redpandadata/redpanda:v24.3.7
    command:
      - redpanda start
      - --smp 1
      - --memory 1G
      - --reserve-memory 0M
      - --node-id 0
      - --check=false
    ports:
      - "127.0.0.1:9092:9092"
    healthcheck:
      test: ["CMD", "rpk", "cluster", "info"]
      interval: 10s
      timeout: 5s
      retries: 10
    restart: unless-stopped
    networks:
      - opsbucket

  postgres:
    container_name: opsbucket-postgres
    image: postgres:16
    environment:
      POSTGRES_USER: opsbucket
      POSTGRES_PASSWORD: {pgpass}
      POSTGRES_DB: opsbucket
    ports:
      - "127.0.0.1:5432:5432"
    volumes:
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U opsbucket -d opsbucket"]
      interval: 5s
      timeout: 5s
      retries: 10
    restart: unless-stopped
    networks:
      - opsbucket

  redis:
    container_name: opsbucket-redis
    image: redis:7-alpine
    ports:
      - "127.0.0.1:6379:6379"
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 5s
      timeout: 3s
      retries: 10
    restart: unless-stopped
    networks:
      - opsbucket

  clickhouse:
    container_name: opsbucket-clickhouse
    image: clickhouse/clickhouse-server:24.3
    environment:
      CLICKHOUSE_USER: default
      CLICKHOUSE_PASSWORD: {chpass}
    ports:
      - "127.0.0.1:8123:8123"
      - "127.0.0.1:9000:9000"
    volumes:
      - chdata:/var/lib/clickhouse
      - ${{PWD}}/clickhouse-schema.sql:/docker-entrypoint-initdb.d/schema.sql
    healthcheck:
      test: ["CMD", "clickhouse-client", "--query", "SELECT 1"]
      interval: 10s
      timeout: 5s
      retries: 10
    restart: unless-stopped
    networks:
      - opsbucket

  minio:
    container_name: opsbucket-minio
    image: minio/minio:latest
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: {minio_key}
      MINIO_ROOT_PASSWORD: {minio_secret}
    ports:
      - "127.0.0.1:9000:9000"
      - "127.0.0.1:9001:9001"
    volumes:
      - miniodata:/data
    healthcheck:
      test: ["CMD-SHELL", "mc alias set local http://localhost:9000 {minio_key} {minio_secret} 2>/dev/null && mc ready local"]
      interval: 10s
      timeout: 5s
      retries: 10
    restart: unless-stopped
    networks:
      - opsbucket

  migrator:
    container_name: opsbucket-migrator
    image: ghcr.io/kings0x/opsbucket/migrator:latest
    environment:
      DATABASE_URL: postgres://opsbucket:{pgpass}@postgres:5432/opsbucket
    depends_on:
      postgres:
        condition: service_healthy
    restart: on-failure
    networks:
      - opsbucket

  ingestion:
    container_name: opsbucket-ingestion
    image: ghcr.io/kings0x/opsbucket/ingestion:latest
    environment:
      KAFKA_BROKERS: redpanda:9092
      DATABASE_URL: postgres://opsbucket:{pgpass}@postgres:5432/opsbucket
      REDIS_URL: redis://redis:6379
      PORT: "8080"
      RUST_LOG: info
    ports:
      - "127.0.0.1:8080:8080"
    depends_on:
      redpanda:
        condition: service_healthy
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    restart: unless-stopped
    networks:
      - opsbucket

  processing:
    container_name: opsbucket-processing
    image: ghcr.io/kings0x/opsbucket/processing:latest
    environment:
      KAFKA_BROKERS: redpanda:9092
      CLICKHOUSE_URL: clickhouse://default:{chpass}@clickhouse:9000/default
      DATABASE_URL: postgres://opsbucket:{pgpass}@postgres:5432/opsbucket
      REDIS_URL: redis://redis:6379
      RUST_LOG: info
    depends_on:
      redpanda:
        condition: service_healthy
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
      clickhouse:
        condition: service_healthy
    restart: unless-stopped
    networks:
      - opsbucket

  archiver:
    container_name: opsbucket-archiver
    image: ghcr.io/kings0x/opsbucket/archiver:latest
    environment:
      CLICKHOUSE_URL: clickhouse://default:{chpass}@clickhouse:9000/default
      S3_ENDPOINT: http://minio:9000
      S3_ACCESS_KEY: {minio_key}
      S3_SECRET_KEY: {minio_secret}
      S3_BUCKET: opsbucket-archive
      RUST_LOG: info
    depends_on:
      clickhouse:
        condition: service_healthy
      minio:
        condition: service_healthy
    restart: unless-stopped
    networks:
      - opsbucket

  query:
    container_name: opsbucket-query
    image: ghcr.io/kings0x/opsbucket/query:latest
    environment:
      SECRET_KEY: {secret_key}
      DATABASE_URL: postgres://opsbucket:{pgpass}@postgres:5432/opsbucket
      REDIS_URL: redis://redis:6379
      CLICKHOUSE_URL: clickhouse://default:{chpass}@clickhouse:9000/default
      PORT: "8081"
      RUST_LOG: info
    ports:
      - "127.0.0.1:8081:8081"
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
      clickhouse:
        condition: service_healthy
    restart: unless-stopped
    networks:
      - opsbucket

  dashboard:
    container_name: opsbucket-dashboard
    image: ghcr.io/kings0x/opsbucket/dashboard:latest
    environment:
      DATABASE_URL: postgres://opsbucket:{pgpass}@postgres:5432/opsbucket
      REDIS_URL: redis://redis:6379
      QUERY_URL: http://query:8081
      PORT: "8082"
      RUST_LOG: info
    ports:
      - "127.0.0.1:8082:8082"
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    restart: unless-stopped
    networks:
      - opsbucket

volumes:
  caddy_data:
  caddy_config:
  pgdata:
  chdata:
  miniodata:

networks:
  opsbucket:
    driver: bridge
"#,
        pgpass = cfg.postgres_password,
        chpass = cfg.clickhouse_password,
        minio_key = cfg.minio_access_key,
        minio_secret = cfg.minio_secret_key,
        secret_key = cfg.secret_key,
    )
}

pub fn generate_caddyfile(cfg: &OpsBucketConfig) -> String {
    format!(
        r#"{domain} {{
    reverse_proxy dashboard:8082
}}

api.{domain} {{
    @ingestion {{
        path /v1/batch
    }}
    reverse_proxy @ingestion ingestion:8080

    @query {{
        path /v1/query/*
    }}
    reverse_proxy @query query:8081

    reverse_proxy dashboard:8082
}}
"#,
        domain = cfg.domain
    )
}

#[allow(dead_code)]
pub fn generate_env(cfg: &OpsBucketConfig) -> String {
    format!(
        r#"DOMAIN={domain}
ADMIN_EMAIL={email}
ADMIN_PASSWORD={password}
SECRET_KEY={sk}
POSTGRES_PASSWORD={pgpass}
CLICKHOUSE_PASSWORD={chpass}
MINIO_ACCESS_KEY={minkey}
MINIO_SECRET_KEY={minsecret}
COMPOSE_PROJECT=opsbucket
"#,
        domain = cfg.domain,
        email = cfg.admin_email,
        password = cfg.admin_password,
        sk = cfg.secret_key,
        pgpass = cfg.postgres_password,
        chpass = cfg.clickhouse_password,
        minkey = cfg.minio_access_key,
        minsecret = cfg.minio_secret_key,
    )
}

pub fn generate_clickhouse_schema() -> &'static str {
    include_str!("../../../infra/clickhouse/schema.sql")
}

pub fn generate_topic_script() -> &'static str {
    "#!/bin/bash
# Create OpsBucket Kafka topics
rpk topic create raw-events --partitions 12
rpk topic create raw-events-dlq --partitions 1
"
}
