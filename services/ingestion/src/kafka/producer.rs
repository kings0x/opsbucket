use anyhow::Result;

pub struct KafkaProducer;

impl KafkaProducer {
    pub fn new(_brokers: &str) -> Result<Self> {
        Ok(Self)
    }
}
