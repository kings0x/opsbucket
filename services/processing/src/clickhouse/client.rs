use anyhow::Result;
use clickhouse::Client as ChClient;

use super::models::ClickHouseRow;

pub async fn insert_batch(client: &ChClient, rows: Vec<ClickHouseRow>) -> Result<()> {
    let mut insert = client.insert("events")?;
    for row in rows {
        insert.write(&row).await?;
    }
    insert.end().await?;
    Ok(())
}
