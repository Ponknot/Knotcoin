// Database modules
pub mod db_common;   // Shared types (AccountState, StoredBlock, etc.)
pub mod db_rocksdb;  // RocksDB implementation (production)
// pub mod db;       // Old sled implementation (kept for reference)

// Re-export main database type
pub use db_rocksdb::ChainDB;
