use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

fn sort_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for v in map.values_mut() {
                sort_keys(v);
            }
            let sorted: serde_json::Map<String, serde_json::Value> =
                std::mem::take(&mut std::mem::replace(map, serde_json::Map::new()));
            *map = sorted;
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                sort_keys(v);
            }
        }
        _ => {}
    }
}

pub fn cache_key<T: Serialize>(spec: &T) -> String {
    let mut value = serde_json::to_value(spec).expect("spec must serialize");
    sort_keys(&mut value);
    let json = serde_json::to_string(&value).expect("canonical json must serialize");
    let hash = Sha256::digest(json.as_bytes());
    format!("qcache:{}", hex::encode(hash))
}

pub async fn get<T: for<'de> Deserialize<'de>>(
    redis: &mut redis::aio::ConnectionManager,
    key: &str,
) -> Result<Option<T>, anyhow::Error> {
    let data: Option<String> = redis.get(key).await?;
    match data {
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        None => Ok(None),
    }
}

pub async fn set<T: Serialize>(
    redis: &mut redis::aio::ConnectionManager,
    key: &str,
    value: &T,
    ttl: u64,
) -> Result<(), anyhow::Error> {
    let json = serde_json::to_string(value)?;
    let _: () = redis.set_ex(key, json, ttl).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn same_spec_different_key_ordering_produces_same_key() {
        let spec1 = json!({"b": 2, "a": 1});
        let spec2 = json!({"a": 1, "b": 2});
        assert_eq!(cache_key(&spec1), cache_key(&spec2));
    }

    #[test]
    fn different_specs_produce_different_keys() {
        let spec1 = json!({"a": 1});
        let spec2 = json!({"a": 2});
        assert_ne!(cache_key(&spec1), cache_key(&spec2));
    }
}
