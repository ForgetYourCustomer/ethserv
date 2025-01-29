use alloy::signers::local::coins_bip39::{English, Mnemonic};
use anyhow::{anyhow, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Serialize, Deserialize)]
struct EncryptedMnemonic {
    encrypted_data: Vec<u8>,
    nonce: Vec<u8>,
    salt: Vec<u8>,
}

pub struct MnemonicStorage {
    storage_path: PathBuf,
}

impl MnemonicStorage {
    pub fn new(storage_path: PathBuf) -> Self {
        Self { storage_path }
    }

    pub fn load_or_create_by_password(&self, password: &str) -> Mnemonic<English> {
        match self.load_mnemonic(password) {
            Ok(mnemonic_words) => mnemonic_words,
            Err(_) => {
                println!("Creating new mnemonic");

                let mut rng = rand::thread_rng();

                let mnemonic: Mnemonic<English> =
                    Mnemonic::new_with_count(&mut rng, 12usize).unwrap();

                let mnemonic_phrase = mnemonic.to_phrase();

                self.save_mnemonic(&mnemonic_phrase, password).unwrap();

                println!("New wallet created and saved");

                self.load_mnemonic(password).unwrap()
            }
        }
    }

    pub fn save_mnemonic(&self, mnemonic: &str, password: &str) -> Result<()> {
        // Validate mnemonic by trying to create a wallet from it

        let salt = {
            let mut salt = [0u8; 32];
            OsRng.fill_bytes(&mut salt);
            salt.to_vec()
        };

        let key = derive_key(password, &salt)?;
        let cipher = ChaCha20Poly1305::new(key.as_ref().into());

        let nonce = {
            let mut nonce = [0u8; 12];
            OsRng.fill_bytes(&mut nonce);
            nonce
        };

        let encrypted_data = cipher
            .encrypt(Nonce::from_slice(&nonce), mnemonic.as_bytes())
            .map_err(|e| anyhow!("Encryption error: {}", e))?;

        let encrypted_mnemonic = EncryptedMnemonic {
            encrypted_data,
            nonce: nonce.to_vec(),
            salt,
        };

        let json = serde_json::to_string(&encrypted_mnemonic)?;
        fs::write(&self.storage_path, json)?;
        Ok(())
    }

    pub fn load_mnemonic(&self, password: &str) -> Result<Mnemonic<English>> {
        let json = fs::read_to_string(&self.storage_path)?;
        let encrypted_mnemonic: EncryptedMnemonic = serde_json::from_str(&json)?;

        let key = derive_key(password, &encrypted_mnemonic.salt)?;
        let cipher = ChaCha20Poly1305::new(key.as_ref().into());

        let decrypted_data = cipher
            .decrypt(
                Nonce::from_slice(&encrypted_mnemonic.nonce),
                encrypted_mnemonic.encrypted_data.as_ref(),
            )
            .map_err(|e| anyhow!("Decryption error: {}", e))?;

        let mnemonic_str = String::from_utf8(decrypted_data)?;

        let mnemonic = Mnemonic::<English>::new_from_phrase(&mnemonic_str).unwrap();

        Ok(mnemonic)
    }
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    use ring::pbkdf2;
    let mut key = [0u8; 32];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        std::num::NonZeroU32::new(100_000).unwrap(),
        salt,
        password.as_bytes(),
        &mut key,
    );
    Ok(key)
}
