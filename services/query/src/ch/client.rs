use clickhouse::Row;

#[derive(Debug)]
pub enum QueryError {
    Timeout,
    ClickHouse(clickhouse::error::Error),
    Other(anyhow::Error),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::Timeout => write!(f, "query timeout"),
            QueryError::ClickHouse(e) => write!(f, "clickhouse error: {}", e),
            QueryError::Other(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for QueryError {}

impl From<clickhouse::error::Error> for QueryError {
    fn from(e: clickhouse::error::Error) -> Self {
        QueryError::ClickHouse(e)
    }
}

pub async fn query<T>(
    client: &clickhouse::Client,
    sql: &str,
    timeout_secs: u64,
) -> Result<Vec<T>, QueryError>
where
    T: Row + serde::de::DeserializeOwned,
{
    let rows: Vec<T> = client
        .query(sql)
        .with_option("max_execution_time", timeout_secs.to_string())
        .fetch_all::<T>()
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("TIMEOUT_EXCEEDED") {
                QueryError::Timeout
            } else {
                QueryError::ClickHouse(e)
            }
        })?;
    Ok(rows)
}
