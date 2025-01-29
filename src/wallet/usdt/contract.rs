use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};

use alloy::{
    eips::BlockNumberOrTag,
    primitives::{address, keccak256, Address, Uint, B256, U256},
    providers::{Provider, RootProvider},
    pubsub::PubSubFrontend,
    rpc::types::Filter,
    sol,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;

use futures_util::StreamExt;

use crate::{config, wallet::database::WalletDatabase};

// Codegen from ABI file to interact with the contract.
sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    IUESDT,
    "src/wallet/usdt/USDT.json"
);

pub async fn get_balance(provider: RootProvider<PubSubFrontend>, owner: &str) -> Result<U256> {
    let usdt_contract_address = config::usdt_contract_address();
    let contract = IUESDT::new(usdt_contract_address, provider);
    let balance = contract
        .balanceOf(Address::from_str(owner)?)
        .call()
        .await?
        ._0;

    return Ok(balance);
}

// Event signature for Transfer(address,address,uint256)
const TRANSFER_EVENT_SIGNATURE: &str = "Transfer(address,address,uint256)";

/// Convert an Ethereum address to a 32-byte topic by zero-padding
fn address_to_topic(address: Address) -> B256 {
    let mut topic_bytes = [0u8; 32];
    topic_bytes[12..32].copy_from_slice(address.as_slice());
    B256::from(topic_bytes)
}

pub async fn get_receive_logs(
    provider: &RootProvider<PubSubFrontend>,
    from_block: Option<u64>,
    to_block: Option<u64>,
    reciever: String,
) -> Result<Vec<TransferLog>> {
    let address = reciever.parse::<Address>()?;
    let topic2 = address_to_topic(address);

    let from_block = match from_block {
        Some(bn) => BlockNumberOrTag::from(bn),
        None => BlockNumberOrTag::Earliest,
    };

    let to_block = match to_block {
        Some(bn) => BlockNumberOrTag::from(bn),
        None => BlockNumberOrTag::Latest,
    };

    let usdt_contract_address = config::usdt_contract_address();

    // Create topic0 for Transfer event
    let filter = Filter::new()
        .address(usdt_contract_address)
        .event(TRANSFER_EVENT_SIGNATURE)
        .topic2(topic2)
        .from_block(from_block)
        .to_block(to_block);

    println!("Filter: {:?}", filter);

    let logs = provider.get_logs(&filter).await?;

    let transfer_logs: Vec<TransferLog> = logs
        .iter()
        .filter_map(|log| parse_transfer_event(log))
        .collect();

    Ok(transfer_logs)
}

pub async fn subscribe_to_transfer_logs(
    provider: &RootProvider<PubSubFrontend>,
    db: Arc<Mutex<WalletDatabase>>,
) -> Result<(tokio::sync::oneshot::Sender<()>, Receiver<TransferLog>)> {
    let (transfer_log_sender, transfer_log_reciever) = tokio::sync::mpsc::channel(1000);

    let (stop_sender, mut stop_receiver) = tokio::sync::oneshot::channel::<()>();

    let usdt_contract_address = config::usdt_contract_address();

    // Build the filter
    let filter = Filter::new()
        .address(usdt_contract_address)
        .event("Transfer(address,address,uint256)");

    let provider = provider.clone();
    let db = db.clone();

    tokio::spawn(async move {
        // Subscribe to logs
        let sub = provider.subscribe_logs(&filter).await.unwrap();
        let mut stream = sub.into_stream();
        let mut block_num = 0;

        while let Some(log) = stream.next().await {
            if let Some(num) = log.block_number {
                if num != block_num {
                    println!("New block: {}", num);
                    block_num = num;
                }
            }
            if let Some(transfer_log) = parse_transfer_event(&log) {
                let should_send = {
                    let db_lock = db.lock().unwrap();
                    // Check if the recipient address exists in our database
                    db_lock.address_exists(&transfer_log.to).unwrap()
                };

                if should_send {
                    println!(
                        "Found transfer to our address: {} USDT from {} to {} at block {}",
                        transfer_log.amount,
                        transfer_log.from,
                        transfer_log.to,
                        transfer_log.block_number
                    );
                    transfer_log_sender.send(transfer_log).await.unwrap();
                }
            }
            if stop_receiver.try_recv().is_ok() {
                println!("Stopping sync");
                break;
            }
        }
    });

    Ok((stop_sender, transfer_log_reciever)) // You'll need to implement proper channel handling here
}

fn parse_transfer_event(log: &alloy::rpc::types::Log) -> Option<TransferLog> {
    // let IUESDT::Transfer { from, to, value }
    match log.log_decode() {
        Ok(decoded) => {
            if let Some(block_num) = decoded.block_number {
                let IUESDT::Transfer { from, to, value } = decoded.inner.data;
                return Some(TransferLog {
                    from: from.to_string(),
                    to: to.to_string(),
                    amount: value,
                    block_number: block_num,
                    hash: decoded.transaction_hash.unwrap().to_string(),
                    index: log.log_index.unwrap(),
                });
            }
            None
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferLog {
    pub from: String,
    pub to: String,
    pub amount: U256,
    pub block_number: u64,
    pub hash: String,
    pub index: u64,
}
