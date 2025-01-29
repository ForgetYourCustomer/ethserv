use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

pub struct WalletDatabase {
    conn: Connection,
}

impl WalletDatabase {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Create addresses table with new schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS addresses (
                id INTEGER PRIMARY KEY,
                address TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL,
                addr_index INTEGER NOT NULL,
                last_used DATETIME,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(path, addr_index)
            )",
            [],
        )?;

        // Create index for faster lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_addresses_path_index ON addresses(path, addr_index)",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn store_address(&self, address: &str, path: &str, addr_index: u32) -> Result<bool> {
        // First check if this path/index combination exists
        let exists = self.conn.query_row(
            "SELECT COUNT(*) FROM addresses WHERE path = ?1 AND addr_index = ?2",
            params![path, addr_index],
            |row| row.get::<_, i32>(0),
        )?;

        if exists > 0 {
            return Ok(false);
        }

        // Try to insert and check if it actually happened
        let changes = self.conn.execute(
            "INSERT INTO addresses (address, path, addr_index) VALUES (?1, ?2, ?3)",
            params![address, path, addr_index],
        )?;

        // Verify that exactly one row was inserted
        Ok(changes == 1)
    }

    pub fn get_max_index_for_path(&self, path: &str) -> Result<Option<u32>> {
        let mut stmt = self
            .conn
            .prepare("SELECT MAX(addr_index) FROM addresses WHERE path = ?1")?;

        let max_index: Option<u32> = stmt.query_row([path], |row| row.get(0))?;
        Ok(max_index)
    }

    pub fn get_address_by_path_and_index(
        &self,
        path: &str,
        addr_index: u32,
    ) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT address FROM addresses WHERE path = ?1 AND addr_index = ?2")?;

        let result = stmt.query_row([path, &addr_index.to_string()], |row| row.get(0));
        match result {
            Ok(address) => Ok(Some(address)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_all_addresses_by_path(&self, path: &str) -> Result<Vec<(String, u32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT address, addr_index FROM addresses WHERE path = ?1 ORDER BY addr_index",
        )?;

        let addresses = stmt
            .query_map([path], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(addresses)
    }

    pub fn update_last_used(&self, address: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE addresses SET last_used = CURRENT_TIMESTAMP WHERE address = ?1",
            params![address],
        )?;
        Ok(())
    }

    pub fn get_address_by_index(&self, addr_index: u32) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT address FROM addresses WHERE addr_index = ?1")?;

        let result = stmt.query_row([addr_index], |row| row.get(0));
        match result {
            Ok(address) => Ok(Some(address)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_all_addresses(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT address, path FROM addresses ORDER BY created_at")?;

        let addresses = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(addresses)
    }

    pub fn get_all_indexes(&self) -> Result<Vec<u32>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT addr_index FROM addresses ORDER BY addr_index")?;

        let indexes = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(indexes)
    }

    pub fn address_exists(&self, address: &str) -> Result<bool> {
        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM addresses WHERE address = ?1")?;

        let count: i64 = stmt.query_row([address], |row| row.get(0))?;
        Ok(count > 0)
    }
}
