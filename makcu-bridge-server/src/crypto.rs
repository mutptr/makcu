use chacha20poly1305::{
    ChaCha20Poly1305, Key, KeyInit,
    aead::{Aead, OsRng, generic_array::GenericArray},
};

pub struct CryptoManager {
    key: Key,
    cipher: ChaCha20Poly1305,
}

impl CryptoManager {
    pub fn new() -> Self {
        let key = ChaCha20Poly1305::generate_key(&mut OsRng);
        let cipher = ChaCha20Poly1305::new(&key);
        Self { key, cipher }
    }

    pub fn decrypt(&self, nonce: &[u8], data: &[u8]) -> anyhow::Result<Vec<u8>> {
        self.cipher
            .decrypt(GenericArray::from_slice(nonce), data)
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn key(&self) -> &[u8] {
        &self.key
    }
}
