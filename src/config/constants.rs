use alloy::primitives::Address;
use config::{Config, ConfigError};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::{env, str::FromStr};

#[derive(Debug, Clone, Copy)]
pub enum Environment {
    Test,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Test => "test",
            Environment::Production => "prod",
        }
    }

    pub fn from_env() -> Self {
        match env::var("ENV")
            .unwrap_or_else(|_| String::from("test"))
            .as_str()
        {
            "prod" => Environment::Production,
            _ => Environment::Test,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub environment: String,
    pub port: u16,
    pub rpc_url: String,
    pub publisher_bind_address: String,
    #[serde(deserialize_with = "deserialize_address")]
    pub usdt_contract_address: Address,
    pub wallet_pw: String,
    pub derivation_path: String,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let environment = Environment::from_env();

        // Load .env file
        dotenv::from_filename(format!(".env.{}", environment.as_str()))
            .expect("Failed to load .env file");

        // Create new config
        let config = Config::builder()
            // Start with default Settings
            .set_default("environment", environment.as_str())?
            // Add in settings from environment variables (with a prefix of APP)
            // E.g. `APP_DEBUG=1 ./target/app` would set the `debug` key
            .add_source(config::Environment::default())
            .build()?;

        // Deserialize configuration
        config.try_deserialize()
    }

    pub fn environment(&self) -> Environment {
        match self.environment.as_str() {
            "prod" => Environment::Production,
            _ => Environment::Test,
        }
    }
}

// Custom deserializer for Address type
fn deserialize_address<'de, D>(deserializer: D) -> Result<Address, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    Address::from_str(&s).map_err(serde::de::Error::custom)
}

// Create a lazy static instance of Settings
lazy_static! {
    pub static ref SETTINGS: Settings = Settings::new().expect("Failed to load settings");
}

// Constants can be accessed through these functions
pub fn environment() -> Environment {
    SETTINGS.environment()
}

pub fn rpc_url() -> &'static str {
    &SETTINGS.rpc_url
}

pub fn usdt_contract_address() -> Address {
    SETTINGS.usdt_contract_address
}

pub fn wallet_pw() -> &'static str {
    &SETTINGS.wallet_pw
}

pub fn port() -> u16 {
    SETTINGS.port
}

pub fn publisher_bind_address() -> &'static str {
    &SETTINGS.publisher_bind_address
}

pub fn derivation_path() -> &'static str {
    &SETTINGS.derivation_path
}
