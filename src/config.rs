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

/// Bind address for P2P — default 0.0.0.0 to allow peer discovery (Public Launch)
/// For anonymous mining, set KNOTCOIN_P2P_BIND=127.0.0.1 to disable external connections
pub const P2P_BIND_ADDRESS_DEFAULT: &str = "0.0.0.0";

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
        let home = resolve_home_dir();
        NetworkConfig {
            p2p_port: P2P_PORT,
            rpc_port: RPC_PORT,
            data_dir: format!("{}/{}", home, DATA_DIR),
        }
    }
}

pub fn p2p_bind_address() -> String {
    let v = std::env::var("KNOTCOIN_P2P_BIND").unwrap_or_else(|_| P2P_BIND_ADDRESS_DEFAULT.to_string());
    if v.trim().is_empty() {
        P2P_BIND_ADDRESS_DEFAULT.to_string()
    } else {
        v
    }
}

pub fn default_data_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(resolve_home_dir()).join(DATA_DIR)
}

fn resolve_home_dir() -> String {
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return home;
        }
    }
    #[cfg(windows)]
    {
        if let Ok(home) = std::env::var("USERPROFILE") {
            if !home.trim().is_empty() {
                return home;
            }
        }
        let drive = std::env::var("HOMEDRIVE").unwrap_or_default();
        let path = std::env::var("HOMEPATH").unwrap_or_default();
        if !drive.is_empty() || !path.is_empty() {
            return format!("{drive}{path}");
        }
    }
    ".".to_string()
}
