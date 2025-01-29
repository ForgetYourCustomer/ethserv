#[cfg(test)]
mod tests {
    use std::{io::Read, str::FromStr, sync::Arc};

    use alloy::{
        eips::BlockNumberOrTag,
        primitives::{address, Address, B256, U256},
        providers::{Provider, ProviderBuilder, RootProvider, WsConnect},
        pubsub::PubSubFrontend,
        rpc::types::{Filter, Topic},
        sol_types::TopicList,
    };
    use futures_util::StreamExt;
    use once_cell::sync::Lazy;
    use tokio::sync::OnceCell;

    static PROVIDER: Lazy<OnceCell<RootProvider<PubSubFrontend>>> = Lazy::new(OnceCell::new);

    // Helper function to parse Transfer event parameters
    fn parse_transfer_event(log: &alloy::rpc::types::Log) -> Option<(Address, Address, U256)> {
        if log.topics().len() < 3 {
            return None;
        }

        // println!("Log: {:?}", log);

        // First topic is event signature, skip it
        // Second topic is 'from' address (32 bytes, but address is last 20 bytes)
        let from = Address::from_slice(&log.topics()[1].as_slice()[12..]);

        // Third topic is 'to' address (32 bytes, but address is last 20 bytes)
        let to = Address::from_slice(&log.topics()[2].as_slice()[12..]);

        let data = &log.data().data;
        // Convert Bytes to fixed-size array
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(data);
        let amount = U256::from_be_bytes(bytes);

        Some((from, to, amount))
    }

    async fn get_provider() -> &'static RootProvider<PubSubFrontend> {
        PROVIDER
            .get_or_init(|| async {
                let provider_url =
                    "wss://eth-mainnet.g.alchemy.com/v2/se25EC3wmH4YT0CiZPRkKsyKBoO6-UOi";
                let ws = WsConnect::new(provider_url);
                ProviderBuilder::new().on_ws(ws).await.unwrap()
            })
            .await
    }

    #[tokio::test]
    async fn get_logs() {
        let provider = get_provider().await;

        // Create a filter to watch for USDT token transfers.
        let usdt_address: Address = address!("0xdAC17F958D2ee523a2206206994597C13D831ec7");

        // Convert address to Topic by padding it to 32 bytes
        let to_address = Address::from_str("0xCBc9745EC4553554938c24B494b4DA9107387695").unwrap();
        let mut topic_bytes = [0u8; 32];
        topic_bytes[12..].copy_from_slice(to_address.as_slice());
        let topic2 = Topic::from(B256::from(topic_bytes));

        let filter = Filter::new()
            .address(usdt_address)
            .event("Transfer(address,address,uint256)")
            .from_block(BlockNumberOrTag::Number(21709710))
            .to_block(BlockNumberOrTag::Latest)
            .topic2(topic2);

        let logs = provider.get_logs(&filter).await.unwrap();

        println!("Filter: {:?}", filter);

        for log in logs {
            if let Some((from, to, amount)) = parse_transfer_event(&log) {
                println!(
                    "Transfer: {} USDT from {} to {} tx_hash: {}",
                    amount,
                    from,
                    to,
                    log.transaction_hash.unwrap()
                );
            }
        }
    }

    #[tokio::test]
    async fn subscribe_logs() {
        let provider = get_provider().await;

        // Create a filter to watch for USDT token transfers.
        let usdt_sepolia_address = address!("0xdAC17F958D2ee523a2206206994597C13D831ec7");
        let filter = Filter::new()
            .address(usdt_sepolia_address)
            .event("Transfer(address,address,uint256)")
            .from_block(BlockNumberOrTag::Latest);

        // Subscribe to logs.
        let sub = provider.subscribe_logs(&filter).await.unwrap();
        let mut stream = sub.into_stream();
        println!("stream created");

        let mut block_num = 0;

        while let Some(log) = stream.next().await {
            if let Some((from, to, amount)) = parse_transfer_event(&log) {
                println!(
                    "Transfer: {} USDT from {} to {} at block {}",
                    amount,
                    from,
                    to,
                    log.block_number.unwrap()
                );
            }

            if let Some(num) = log.block_number {
                if num != block_num {
                    println!("New block: {}", num);
                    block_num = num;
                }
            }
        }
    }
}
