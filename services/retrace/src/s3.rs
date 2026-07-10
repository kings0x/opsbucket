use anyhow::Result;
use chrono::Utc;
use hex::encode as hex_encode;
use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

pub struct S3Store {
    client: Client,
    endpoint: String,
    bucket: String,
    region: String,
    access_key: String,
    secret_key: String,
}

impl S3Store {
    pub async fn new(
        endpoint: &str,
        region: &str,
        access_key: &str,
        secret_key: &str,
        bucket: &str,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        let store = Self {
            client,
            endpoint: endpoint.trim_end_matches('/').to_string(),
            bucket: bucket.to_string(),
            region: region.to_string(),
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
        };

        store.ensure_bucket().await?;
        Ok(store)
    }

    pub async fn upload_chunk(
        &self,
        project_id: &str,
        session_id: &str,
        chunk_seq: u32,
        data: &[u8],
    ) -> Result<String> {
        let key = format!("replay/{}/{}/{}.json", project_id, session_id, chunk_seq);
        let url = format!("{}/{}/{}", self.endpoint, self.bucket, key);

        let body_hash = sha256_hex(data);
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();

        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request = format!(
            "PUT\n/{}/{}\n\nhost:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n\n{}\n{}",
            self.bucket, key, self.host(), body_hash, amz_date, signed_headers, body_hash
        );

        let algorithm = "AWS4-HMAC-SHA256";
        let credential_scope = format!("{}/{}/s3/aws4_request", date_stamp, self.region);
        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm,
            amz_date,
            credential_scope,
            sha256_hex(canonical_request.as_bytes())
        );

        let signing_key = self.signing_key(&date_stamp);
        let signature = hex_encode(
            HmacSha256::new_from_slice(&signing_key)
                .unwrap()
                .chain_update(string_to_sign.as_bytes())
                .finalize()
                .into_bytes(),
        );

        let authorization = format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, self.access_key, credential_scope, signed_headers, signature
        );

        let response = self
            .client
            .put(&url)
            .header("x-amz-content-sha256", &body_hash)
            .header("x-amz-date", &amz_date)
            .header("Authorization", &authorization)
            .header("Content-Type", "application/json")
            .body(data.to_vec())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("s3 upload failed: status={}, body={}", status, body);
        }

        Ok(key)
    }

    async fn ensure_bucket(&self) -> Result<()> {
        let url = format!("{}/{}", self.endpoint, self.bucket);
        let head_resp = self.client.head(&url).send().await?;
        if head_resp.status().is_success() {
            return Ok(());
        }

        if head_resp.status().as_u16() == 403 {
            return Ok(());
        }

        let put_resp = self
            .client
            .put(&url)
            .header("x-amz-content-sha256", sha256_hex(b""))
            .header("x-amz-date", Utc::now().format("%Y%m%dT%H%M%SZ").to_string())
            .body("")
            .send()
            .await?;

        if !put_resp.status().is_success() && put_resp.status().as_u16() != 409 {
            let status = put_resp.status();
            let body = put_resp.text().await.unwrap_or_default();
            anyhow::bail!("s3 bucket creation failed: status={}, body={}", status, body);
        }

        Ok(())
    }

    fn host(&self) -> &str {
        self.endpoint
            .strip_prefix("http://")
            .or_else(|| self.endpoint.strip_prefix("https://"))
            .unwrap_or(&self.endpoint)
    }

    fn signing_key(&self, date_stamp: &str) -> Vec<u8> {
        let k_secret = format!("AWS4{}", self.secret_key);
        let k_date = hmac_sha256(k_secret.as_bytes(), date_stamp.as_bytes());
        let k_region = hmac_sha256(&k_date, self.region.as_bytes());
        let k_service = hmac_sha256(&k_region, b"s3");
        hmac_sha256(&k_service, b"aws4_request")
    }
}

fn sha256_hex(data: &[u8]) -> String {
    hex_encode(Sha256::digest(data))
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    HmacSha256::new_from_slice(key)
        .unwrap()
        .chain_update(data)
        .finalize()
        .into_bytes()
        .to_vec()
}

#[cfg(test)]
pub struct MockS3Store {
    pub chunks: std::sync::Mutex<Vec<(String, String, u32, Vec<u8>)>>,
    pub should_fail: bool,
}

#[cfg(test)]
impl MockS3Store {
    pub fn new() -> Self {
        Self {
            chunks: std::sync::Mutex::new(Vec::new()),
            should_fail: false,
        }
    }

    pub fn with_failure() -> Self {
        Self {
            chunks: std::sync::Mutex::new(Vec::new()),
            should_fail: true,
        }
    }

    pub async fn upload_chunk(
        &self,
        project_id: &str,
        session_id: &str,
        chunk_seq: u32,
        data: &[u8],
    ) -> Result<String> {
        if self.should_fail {
            anyhow::bail!("s3 upload failed (mock)");
        }
        let key = format!("replay/{}/{}/{}.json", project_id, session_id, chunk_seq);
        self.chunks
            .lock()
            .unwrap()
            .push((
                project_id.to_string(),
                session_id.to_string(),
                chunk_seq,
                data.to_vec(),
            ));
        Ok(key)
    }
}
