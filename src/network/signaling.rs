use anyhow::Result;
use tokio::net::TcpStream;

pub struct SignalingClient {
    server_addr: String,
}

impl SignalingClient {
    pub fn new(server_addr: &str) -> Self {
        Self {
            server_addr: server_addr.to_string(),
        }
    }

    pub async fn register(&self, peer_id: &str) -> Result<()> {
        let _stream = TcpStream::connect(&self.server_addr).await?;
        tracing::info!("Registered peer: {peer_id} with signaling server");
        Ok(())
    }
}
