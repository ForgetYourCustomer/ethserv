use std::sync::Arc;

use alloy::{primitives::U256, providers::RootProvider, pubsub::PubSubFrontend};
use axum::{
    extract::{FromRef, Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    pubsub::ChainEvent,
    wallet::usdt::contract::{get_balance, get_receive_logs, TransferLog},
    EthServWallet,
};

#[derive(Serialize)]
pub struct AddressResponse {
    success: bool,
    address: String,
    error: Option<String>,
}

#[derive(Serialize)]
pub struct BalanceResponse {
    success: bool,
    balance: String,
    error: Option<String>,
}

async fn get_balance_controller(
    State(wallet): State<Arc<EthServWallet>>,
    Path(address): Path<String>,
) -> (StatusCode, Json<BalanceResponse>) {
    println!("Getting balance");
    let response = match get_balance(wallet.provider.clone(), address.as_str()).await {
        Ok(balance) => (
            StatusCode::OK,
            Json(BalanceResponse {
                success: true,
                balance: balance.to_string(),
                error: None,
            }),
        ),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BalanceResponse {
                success: false,
                balance: String::from("0"),
                error: Some(format!("{:?}", _e)),
            }),
        ),
    };
    response
}

async fn get_new_address(
    State(wallet): State<Arc<EthServWallet>>,
) -> (StatusCode, Json<AddressResponse>) {
    println!("Getting new address");
    let response = match wallet.reveal_next_address() {
        Ok(address) => (
            StatusCode::OK,
            Json(AddressResponse {
                success: true,
                address,
                error: None,
            }),
        ),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AddressResponse {
                success: false,
                address: String::new(),
                error: Some(String::from("Error getting new address")),
            }),
        ),
    };
    response
}

async fn get_address_deposits(
    State(wallet): State<Arc<EthServWallet>>,
    Json(tx): Json<AddressDepositsRequest>,
) -> (StatusCode, Json<AddressDepositsResponse>) {
    println!("Get address req");

    let transfer_logs =
        get_receive_logs(&wallet.provider, tx.start_block, tx.end_block, tx.address)
            .await
            .unwrap();

    (
        StatusCode::OK,
        Json(AddressDepositsResponse {
            deposits: transfer_logs,
        }),
    )
}

async fn test_pub_deposits(
    State(wallet): State<Arc<EthServWallet>>,
    Json(tx): Json<TestPubTxRequest>,
) -> (StatusCode, Json<TestPubTxResponse>) {
    println!("Testing public transaction");

    for action in tx.actions {
        println!("Action: {:?}", action);
        let chain_event = ChainEvent::NewDeposit { deposit: action };
        if let Err(e) = wallet.publish_chainevent(chain_event) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TestPubTxResponse {
                    success: false,
                    error: Some(format!("Error publishing chain event: {:?}", e)),
                }),
            );
        }
    }
    (
        StatusCode::OK,
        Json(TestPubTxResponse {
            success: true,
            error: None,
        }),
    )
}

// Create router
pub fn create_router(wallet: Arc<EthServWallet>) -> Router {
    Router::new()
        .route("/balance/:address", get(get_balance_controller))
        .route("/new-address", get(get_new_address))
        .route("/address-deposits", post(get_address_deposits))
        .route("/test/pub-deposits", post(test_pub_deposits))
        .with_state(wallet)
}

#[derive(Deserialize)]
struct TestPubTxRequest {
    actions: Vec<(String, String, u64, String, u64)>,
}

#[derive(Serialize)]
struct TestPubTxResponse {
    success: bool,
    error: Option<String>,
}

#[derive(Deserialize)]
struct AddressDepositsRequest {
    address: String,
    start_block: Option<u64>,
    end_block: Option<u64>,
}

#[derive(Serialize)]
struct AddressDepositsResponse {
    deposits: Vec<TransferLog>,
}
