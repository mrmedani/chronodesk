use anyhow::Result;

pub struct Transport {
}

impl Transport {
    pub fn new() -> Self {
        Self { }
    }

    pub async fn connect(&mut self, peer_id: &str) -> Result<()> {
        tracing::info!("Connecting to peer: {peer_id}");
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }
}
