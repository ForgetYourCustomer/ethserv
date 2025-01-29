use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zmq::{Context, Socket};

#[derive(Serialize, Deserialize, Debug)]
pub enum ChainEvent {
    #[serde(rename = "newtx")]
    NewTransaction {
        txid: String,
        amount: i64,
        confirmations: u32,
    },
    NewAddress {
        address: String,
    },
    #[serde(rename = "dpst")]
    NewDeposit {
        deposit: (String, String, u64, String, u64), // (address, amount, block_number, hash, index)
    },
}

pub struct Publisher {
    socket: Socket,
}

impl Publisher {
    pub fn new(bind_address: &str) -> Result<Self> {
        let context = Context::new();
        let socket = context.socket(zmq::PUB)?;
        socket.bind(bind_address)?;
        Ok(Self { socket })
    }

    pub fn publish(&self, event: ChainEvent) -> Result<()> {
        let message = serde_json::to_string(&event)?;
        // Send topic first
        self.socket.send("tx", zmq::SNDMORE)?;
        // Then send the actual message
        self.socket.send(&message, 0)?;
        Ok(())
    }
}

// Helper function to create a publisher instance
pub fn create_publisher() -> Arc<Publisher> {
    let publisher = Publisher::new("tcp://*:5556").expect("Failed to create ZMQ publisher");
    Arc::new(publisher)
}
