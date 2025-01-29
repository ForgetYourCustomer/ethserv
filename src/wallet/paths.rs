use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Debug)]
pub struct WalletPaths {
    pub mnemonic_path: PathBuf,
    pub wallet_path: PathBuf,
}

impl WalletPaths {
    pub fn from_password(password: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let result = hasher.finalize();
        let hash = hex::encode(&result[..16]); // Use first 16 bytes for shorter filename

        let pers_dir = PathBuf::from("pers");
        std::fs::create_dir_all(&pers_dir).expect("Failed to create persistence directory");

        let mnemonic_path = pers_dir.join(format!("mnemonic_{}.dat", hash));
        let wallet_path = pers_dir.join(format!("wallet_{}.sqlite", hash));

        Self {
            mnemonic_path,
            wallet_path,
        }
    }
}
