use anyhow::Result;

#[allow(dead_code)]
pub struct Crypto {
    key: [u8; 32],
    algorithm: &'static str,
}

impl Crypto {
    pub fn new() -> Self {
        Self {
            key: [0u8; 32],
            algorithm: "none",
        }
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }
}

impl Default for Crypto {
    fn default() -> Self {
        Self::new()
    }
}
