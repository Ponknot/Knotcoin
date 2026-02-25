/// Standard Protocol Ports
pub const P2P_PORT: u16 = 9000;
pub const RPC_PORT: u16 = 9001;
pub const EXPLORER_PORT: u16 = 3000;

/// Maximum number of peers to connect to
pub const MAX_PEERS: usize = 12;

/// Maximum message size (1 MB)
pub const MAX_MESSAGE_SIZE: usize = 1_048_576;

/// Bind address for RPC — set to 127.0.0.1 for local-only access (Security)
pub const RPC_BIND_ADDRESS: &str = "127.0.0.1";

/// RPC authentication cookie filename
pub const RPC_COOKIE_FILE: &str = ".cookie";

/// Bind address for P2P — set to 0.0.0.0 to allow peer discovery (Public Launch)
pub const P2P_BIND_ADDRESS: &str = "0.0.0.0";

/// Data directory names
/// Data directory name
pub const DATA_DIR: &str = ".knotcoin/mainnet";

pub struct NetworkConfig {
    pub p2p_port: u16,
    pub rpc_port: u16,
    pub data_dir: String,
}

impl NetworkConfig {
    pub fn mainnet() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        NetworkConfig {
            p2p_port: P2P_PORT,
            rpc_port: RPC_PORT,
            data_dir: format!("{}/{}", home, DATA_DIR),
        }
    }
}
