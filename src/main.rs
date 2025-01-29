use alloy::{providers::ProviderBuilder, transports::ws::WsConnect};
use anyhow::Result;
use log::info;
use std::sync::Arc;

use ethserv::{api::create_router, config, EthServWallet};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the logger
    env_logger::init();
    info!("Starting BitServ wallet application...");

    let port = config::port();
    let rpc_url = config::rpc_url();

    let wallet_pw = config::wallet_pw();

    let ws = WsConnect::new(rpc_url);

    let provider = ProviderBuilder::new().on_ws(ws).await?;

    let mut wallet = EthServWallet::new(wallet_pw, provider)?;

    wallet.start_sync();

    // // Create router
    let wallet = Arc::new(wallet);
    let app = create_router(wallet);

    let bind_address = format!("127.0.0.1:{}", port);

    // // Run server
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;

    println!("Server running on {}", &bind_address);
    axum::serve(listener, app).await?;

    Ok(())
}
