use std::sync::{Arc, Mutex};

use alloy::{
    providers::RootProvider,
    pubsub::PubSubFrontend,
    signers::local::{
        coins_bip39::{English, Mnemonic},
        MnemonicBuilder,
    },
};
use anyhow::Result;
use tokio::sync::oneshot;

use crate::{config, pubsub::ChainEvent, Publisher};

use super::{
    database::WalletDatabase, mnemonic::MnemonicStorage, paths::WalletPaths, usdt::contract,
};

pub struct EthServWallet {
    mnemonic: Mnemonic<English>,
    db: Arc<Mutex<WalletDatabase>>,
    pub provider: RootProvider<PubSubFrontend>,
    is_syncing: bool,
    stop_sync_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    publisher: Arc<Mutex<Publisher>>,
}

const DERIVATION_PATH: &str = "m/44'/60'/0'/0/";

impl EthServWallet {
    pub fn new(password: &str, provider: RootProvider<PubSubFrontend>) -> Result<Self> {
        let paths = WalletPaths::from_password(password);
        let mnemonic_storage = MnemonicStorage::new(paths.mnemonic_path);

        let mnemonic = mnemonic_storage.load_or_create_by_password(password);
        let db = WalletDatabase::new(paths.wallet_path)?;

        let publisher_bind_address = config::publisher_bind_address();

        let publisher = Arc::new(Mutex::new(Publisher::new(publisher_bind_address).unwrap()));

        Ok(Self {
            mnemonic,
            db: Arc::new(Mutex::new(db)),
            provider,
            is_syncing: false,
            stop_sync_tx: Arc::new(Mutex::new(None)),
            publisher,
        })
    }

    pub fn start_sync(&mut self) {
        if self.is_syncing {
            println!("Sync already in progress");
            return;
        }
        self.is_syncing = true;

        let provider = self.provider.clone();

        let db = self.db.clone();

        let publisher = self.publisher.clone();

        let stop_sync_tx = self.stop_sync_tx.clone();

        println!("Subscribing to transfer logs...");

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let (a, mut b) = contract::subscribe_to_transfer_logs(&provider, db.clone())
                    .await
                    .unwrap();

                *stop_sync_tx.lock().unwrap() = Some(a);

                while let Some(transfer) = b.recv().await {
                    println!("Transfer: {:?}", transfer);
                    let chain_event = ChainEvent::NewDeposit {
                        deposit: (
                            transfer.to,
                            transfer.amount.to_string(),
                            transfer.block_number,
                            transfer.hash,
                            transfer.index,
                        ),
                    };

                    publisher.lock().unwrap().publish(chain_event).unwrap();
                }
            });
        });
    }

    pub fn stop_sync(&mut self) {
        if let Some(tx) = self.stop_sync_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
        self.is_syncing = false;
    }

    pub fn reveal_next_address(&self) -> Result<String> {
        let derivation_path = config::derivation_path();
        let db_lock = self.db.lock().unwrap();
        let last_index = db_lock
            .get_max_index_for_path(derivation_path)?
            .unwrap_or(0);

        println!("Last index: {}", last_index);

        let new_index = last_index + 1;

        let derivation_path = format!("{}{}", derivation_path, new_index);

        println!("Derivation path: {}", derivation_path);

        let wallet = MnemonicBuilder::<English>::default()
            .phrase(self.mnemonic.to_phrase())
            .derivation_path(derivation_path)?
            .build()?;

        let address = wallet.address().to_string();
        match db_lock.store_address(&address, DERIVATION_PATH, new_index) {
            Ok(true) => Ok(address),
            Ok(false) => Err(anyhow::anyhow!("Address already exists")),
            Err(e) => Err(anyhow::anyhow!(e)),
        }
    }

    pub fn publish_chainevent(&self, event: ChainEvent) -> Result<()> {
        let publisher = self.publisher.lock().unwrap();
        publisher.publish(event)
    }
}
