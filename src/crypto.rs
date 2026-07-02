use anyhow::{anyhow, Result};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
use ring::agreement::{agree_ephemeral, EphemeralPrivateKey, UnparsedPublicKey, X25519};
use ring::hkdf::{Salt, HKDF_SHA256};
use ring::rand::{SecureRandom, SystemRandom};

const NONCE_LEN: usize = 12;
pub const KEY_LEN: usize = 32;

#[derive(Clone)]
pub struct CryptoSession {
    key: LessSafeKey,
    rng: SystemRandom,
}

impl CryptoSession {
    pub fn new(key_bytes: &[u8; KEY_LEN]) -> Result<Self> {
        let unbound = UnboundKey::new(&CHACHA20_POLY1305, key_bytes)
            .map_err(|e| anyhow!("failed to create cipher key: {e:?}"))?;
        Ok(Self {
            key: LessSafeKey::new(unbound),
            rng: SystemRandom::new(),
        })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        self.rng
            .fill(&mut nonce_bytes)
            .map_err(|e| anyhow!("failed to generate nonce: {e:?}"))?;
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);

        let mut in_out = plaintext.to_vec();
        self.key
            .seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
            .map_err(|e| anyhow!("encryption failed: {e:?}"))?;

        let mut result = Vec::with_capacity(NONCE_LEN + in_out.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&in_out);
        Ok(result)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() <= NONCE_LEN {
            return Err(anyhow!("encrypted data too short"));
        }
        let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
        let nonce = Nonce::assume_unique_for_key(
            nonce_bytes
                .try_into()
                .map_err(|_| anyhow!("invalid nonce length"))?,
        );
        let mut in_out = ciphertext.to_vec();
        let plaintext = self
            .key
            .open_in_place(nonce, Aad::empty(), &mut in_out)
            .map_err(|e| anyhow!("decryption failed: {e:?}"))?;
        Ok(plaintext.to_vec())
    }
}

pub fn generate_keypair() -> Result<(EphemeralPrivateKey, Vec<u8>)> {
    let rng = SystemRandom::new();
    let private_key = EphemeralPrivateKey::generate(&X25519, &rng)
        .map_err(|e| anyhow!("key generation failed: {e:?}"))?;
    let public_key = private_key
        .compute_public_key()
        .map_err(|e| anyhow!("failed to compute public key: {e:?}"))?;
    let public_key_bytes = public_key.as_ref().to_vec();
    Ok((private_key, public_key_bytes))
}

pub fn compute_shared_secret(
    private_key: EphemeralPrivateKey,
    peer_public_key_bytes: &[u8],
) -> Result<Vec<u8>> {
    let peer_public = UnparsedPublicKey::new(&X25519, peer_public_key_bytes);
    let shared_secret = agree_ephemeral(private_key, &peer_public, |key_material| {
        key_material.to_vec()
    })
    .map_err(|e| anyhow!("key agreement failed: {e:?}"))?;
    Ok(shared_secret)
}

pub fn derive_session_key(shared_secret: &[u8]) -> Result<[u8; KEY_LEN]> {
    let salt = Salt::new(HKDF_SHA256, &[]);
    let prk = salt.extract(shared_secret);
    let okm = prk
        .expand(&[b"chronodesk-session-key"], HKDF_SHA256)
        .map_err(|e| anyhow!("key derivation expand failed: {e:?}"))?;
    let mut key = [0u8; KEY_LEN];
    okm.fill(&mut key)
        .map_err(|e| anyhow!("key derivation fill failed: {e:?}"))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_handshake_keys_match() {
        let (host_priv, host_pub) = generate_keypair().expect("host keypair");
        let (viewer_priv, viewer_pub) = generate_keypair().expect("viewer keypair");

        let host_shared =
            compute_shared_secret(host_priv, &viewer_pub).expect("host shared secret");
        let viewer_shared =
            compute_shared_secret(viewer_priv, &host_pub).expect("viewer shared secret");

        assert_eq!(
            host_shared, viewer_shared,
            "both sides must compute the same shared secret"
        );

        let host_key = derive_session_key(&host_shared).expect("host session key");
        let viewer_key = derive_session_key(&viewer_shared).expect("viewer session key");

        assert_eq!(
            host_key, viewer_key,
            "both sides must derive the same session key"
        );
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [0xABu8; KEY_LEN];
        let session = CryptoSession::new(&key).expect("session");

        let plaintext = b"Hello, CHRONODESK! This is a secret message.";
        let ciphertext = session.encrypt(plaintext).expect("encrypt");
        let decrypted = session.decrypt(&ciphertext).expect("decrypt");

        assert_eq!(
            &decrypted, plaintext,
            "decrypted must match original plaintext"
        );
        assert_ne!(
            ciphertext, plaintext,
            "ciphertext must differ from plaintext"
        );
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let key = [0x42u8; KEY_LEN];
        let session = CryptoSession::new(&key).expect("session");

        let plaintext = b"";
        let ciphertext = session.encrypt(plaintext).expect("encrypt empty");
        let decrypted = session.decrypt(&ciphertext).expect("decrypt empty");

        assert_eq!(decrypted.len(), 0, "empty plaintext roundtrip");
    }

    #[test]
    fn test_encrypt_decrypt_large() {
        let key = [0xCDu8; KEY_LEN];
        let session = CryptoSession::new(&key).expect("session");

        let plaintext = vec![0xBBu8; 100_000];
        let ciphertext = session.encrypt(&plaintext).expect("encrypt large");
        let decrypted = session.decrypt(&ciphertext).expect("decrypt large");

        assert_eq!(decrypted, plaintext, "large payload roundtrip");
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = [0x01u8; KEY_LEN];
        let session = CryptoSession::new(&key).expect("session");

        let plaintext = b"tamper test";
        let mut ciphertext = session.encrypt(plaintext).expect("encrypt");

        if ciphertext.len() > 13 {
            ciphertext[13] ^= 0xFF;
        }

        let result = session.decrypt(&ciphertext);
        assert!(result.is_err(), "tampered ciphertext must fail decryption");
    }

    #[test]
    fn test_wrong_key_fails() {
        let key_alice = [0xAAu8; KEY_LEN];
        let key_bob = [0xBBu8; KEY_LEN];
        let alice = CryptoSession::new(&key_alice).expect("alice");
        let bob = CryptoSession::new(&key_bob).expect("bob");

        let ciphertext = alice.encrypt(b"secret from alice").expect("encrypt");
        let result = bob.decrypt(&ciphertext);

        assert!(result.is_err(), "decrypting with wrong key must fail");
    }

    #[test]
    fn test_too_short_data_fails() {
        let key = [0xCCu8; KEY_LEN];
        let session = CryptoSession::new(&key).expect("session");

        let result = session.decrypt(b"");
        assert!(result.is_err(), "empty data must fail");

        let result = session.decrypt(b"short");
        assert!(result.is_err(), "data shorter than nonce must fail");

        let result = session.decrypt(&[0u8; 12]);
        assert!(result.is_err(), "data exactly nonce length must fail");
    }

    #[test]
    fn test_message_serialization_flow() {
        use crate::protocol::ChannelMessage;

        let key = [0xEFu8; KEY_LEN];
        let session = CryptoSession::new(&key).expect("session");

        let original = ChannelMessage::InputMove { x: 1920, y: 1080 };
        let serialized = bincode::serialize(&original).expect("serialize");
        let ciphertext = session.encrypt(&serialized).expect("encrypt");

        let decrypted = session.decrypt(&ciphertext).expect("decrypt");
        let deserialized: ChannelMessage = bincode::deserialize(&decrypted).expect("deserialize");

        match deserialized {
            ChannelMessage::InputMove { x, y } => {
                assert_eq!(x, 1920);
                assert_eq!(y, 1080);
            }
            _ => panic!("wrong message type after roundtrip"),
        }
    }

    #[test]
    fn test_unique_nonces_per_encryption() {
        let key = [0xDDu8; KEY_LEN];
        let session = CryptoSession::new(&key).expect("session");
        let plaintext = b"same plaintext";

        let c1 = session.encrypt(plaintext).expect("encrypt 1");
        let c2 = session.encrypt(plaintext).expect("encrypt 2");

        let nonce1 = &c1[..NONCE_LEN];
        let nonce2 = &c2[..NONCE_LEN];
        assert_ne!(nonce1, nonce2, "each encryption must use a different nonce");
        assert_ne!(c1, c2, "ciphertexts must differ even with same plaintext");
    }

    #[test]
    fn test_full_e2e_handshake_and_messages() {
        use crate::protocol::ChannelMessage;
        let (host_priv, host_pub) = generate_keypair().expect("host keypair");
        let (viewer_priv, viewer_pub) = generate_keypair().expect("viewer keypair");

        let host_key = {
            let shared = compute_shared_secret(host_priv, &viewer_pub).expect("host shared");
            derive_session_key(&shared).expect("host key")
        };
        let viewer_key = {
            let shared = compute_shared_secret(viewer_priv, &host_pub).expect("viewer shared");
            derive_session_key(&shared).expect("viewer key")
        };
        assert_eq!(host_key, viewer_key);

        let host_session = CryptoSession::new(&host_key).expect("host session");
        let viewer_session = CryptoSession::new(&viewer_key).expect("viewer session");

        let msgs = vec![
            ChannelMessage::InputMove { x: 100, y: 200 },
            ChannelMessage::InputClick {
                button: 1,
                pressed: true,
            },
            ChannelMessage::InputKey {
                key: 42,
                pressed: false,
            },
            ChannelMessage::VideoFrame {
                width: 1920,
                height: 1080,
                codec: 2,
                data: vec![0; 1024],
            },
            ChannelMessage::AudioData {
                data: vec![1, 2, 3],
                sample_rate: 48000,
                channels: 2,
            },
            ChannelMessage::Clipboard {
                text: "hello".to_string(),
            },
            ChannelMessage::Ping { timestamp: 12345 },
        ];

        for original in &msgs {
            let data = bincode::serialize(original).expect("serialize");
            let ciphertext = host_session.encrypt(&data).expect("encrypt");
            let decrypted = viewer_session.decrypt(&ciphertext).expect("decrypt");
            let recovered: ChannelMessage = bincode::deserialize(&decrypted).expect("deserialize");
            assert_eq!(format!("{original:?}"), format!("{recovered:?}"));
        }
    }

    #[test]
    fn test_multiple_sessions_independent() {
        let key1 = [0x11u8; KEY_LEN];
        let key2 = [0x22u8; KEY_LEN];
        let s1 = CryptoSession::new(&key1).expect("session 1");
        let s2 = CryptoSession::new(&key2).expect("session 2");

        let msg1 = s1.encrypt(b"hello").expect("s1 encrypt");
        let msg2 = s2.encrypt(b"hello").expect("s2 encrypt");

        let dec1 = s1.decrypt(&msg1).expect("s1 decrypt own");
        let dec2 = s2.decrypt(&msg2).expect("s2 decrypt own");
        assert_eq!(dec1, b"hello");
        assert_eq!(dec2, b"hello");
    }
}
