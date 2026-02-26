use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::Mutex;

use knotcoin::config::NetworkConfig;
use knotcoin::consensus::genesis::create_genesis_block;
use knotcoin::consensus::state::apply_block;
use knotcoin::net::mempool::Mempool;
use knotcoin::net::node::P2PNode;
use knotcoin::node::ChainDB;
use knotcoin::rpc::server::{RpcState, start_rpc_server};

use colored::*;

fn banner() {
    println!(
        "{}",
        " ██╗  ██╗███╗   ██╗██████╗ ████████╗ ██████╗██████╗ ███╗   ██╗".bright_cyan()
    );
    println!(
        "{}",
        " ██║ ██╔╝████╗  ██║██╔══██╗╚══██╔══╝██╔════╝██╔══██╗████╗  ██║".bright_cyan()
    );
    println!(
        "{}",
        " █████╔╝ ██╔██╗ ██║██║  ██║   ██║   ██║     ██║  ██║██╔██╗ ██║"
            .bright_cyan()
            .bold()
    );
    println!(
        "{}",
        " ██╔═██╗ ██║╚██╗██║██║  ██║   ██║   ██║     ██║  ██║██║╚██╗██║"
            .bright_cyan()
            .bold()
    );
    println!(
        "{}",
        " ██║  ██╗██║ ╚████║██████╔╝   ██║   ╚██████╗██████╔╝██║ ╚████║".blue()
    );
    println!(
        "{}",
        " ╚═╝  ╚═╝╚═╝  ╚═══╝╚═════╝    ╚═╝    ╚═════╝╚═════╝ ╚═╝  ╚═══╝".blue()
    );
    println!();
    println!(
        "{}",
        "                    v1.0.1 MAINNET                       "
            .bright_green()
            .on_black()
            .bold()
    );
    println!(
        "{}",
        "         Quantum-Secure Electronic Cash System           "
            .bright_green()
            .bold()
    );
    println!();
    println!("{}", " [SECURITY WARNING] ".on_red().white().bold());
    println!(
        "{}",
        " Your public IP will be visible to peers you connect with.".red()
    );
    println!("{}", " Use a VPN or Tor for absolute anonymity.".red());
    println!();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    banner();

    let mut config = NetworkConfig::mainnet();

    // Allow environment overrides for multi-node testing
    if let Ok(p) = std::env::var("KNOTCOIN_RPC_PORT")
        && let Ok(port) = p.parse()
    {
        config.rpc_port = port;
    }
    if let Ok(p) = std::env::var("KNOTCOIN_P2P_PORT")
        && let Ok(port) = p.parse()
    {
        config.p2p_port = port;
    }
    if let Ok(d) = std::env::var("KNOTCOIN_DATA_DIR") {
        config.data_dir = d;
    }

    println!(
        "{} data dir: {}",
        "[init]".bright_blue().bold(),
        config.data_dir
    );
    std::fs::create_dir_all(&config.data_dir)?;

    let db = ChainDB::open(&PathBuf::from(&config.data_dir).join("chaindata"))?;
    println!("{} chain database opened", "[init]".bright_blue().bold());

    if db.get_tip()?.is_none() {
        println!(
            "{} empty chain — applying genesis block",
            "[init]".bright_blue().bold()
        );
        apply_block(&db, &create_genesis_block())?;
    }

    println!(
        "{} chain height: {}",
        "[init]".bright_blue().bold(),
        db.get_chain_height()?
    );

    let (p2p_tx, p2p_rx) = tokio::sync::mpsc::unbounded_channel();

    // SECURITY: Generate RPC authentication token
    let auth_token = knotcoin::rpc::server::generate_rpc_auth_token(&config.data_dir)?;
    println!(
        "{} RPC auth token: {}",
        "[security]".bright_yellow().bold(),
        &auth_token[..16]
    );
    println!(
        "{}",
        format!("           Full token saved to: {}/.cookie", config.data_dir).yellow()
    );

    let state = Arc::new(RpcState {
        db,
        mempool: Arc::new(Mutex::new(Mempool::new())),
        shutdown: AtomicBool::new(false),
        p2p_tx,
        auth_token,
    });

    let p2p_state = state.clone();
    let p2p_port = config.p2p_port;
    tokio::spawn(async move {
        let node = P2PNode::new_from_rpc_state(p2p_state);
        // Ensure mempool is shared between RPC and P2P
        // node.mempool = p2p_state.mempool.clone(); // If needed
        if let Err(e) = node.start_on_port(p2p_port, p2p_rx).await {
            eprintln!("{} error: {e}", "[p2p]".bright_red().bold());
        }
    });

    println!(
        "{} RPC server listening on {}:{}",
        "[rpc] ".bright_magenta().bold(),
        knotcoin::config::RPC_BIND_ADDRESS,
        config.rpc_port
    );
    println!(
        "{} P2P server listening on {}:{}",
        "[p2p] ".bright_green().bold(),
        knotcoin::config::P2P_BIND_ADDRESS,
        config.p2p_port
    );
    println!();
    println!(
        "{}",
        "  Usage: knotcoin-cli <command> [args...]"
            .bright_yellow()
            .bold()
    );
    println!("  {} knotcoin-cli getblockcount", "❯".bright_black());
    println!(
        "  {} knotcoin-cli generatetoaddress 10 <hex_address>",
        "❯".bright_black()
    );
    println!(
        "  {} knotcoin-cli getbalance <hex_address>",
        "❯".bright_black()
    );
    println!("  {} knotcoin-cli addnode <ip:port>", "❯".bright_black());
    println!("  {} knotcoin-cli stop", "❯".bright_black());
    println!();

    start_rpc_server(state, config.rpc_port).await?;
    println!("{} done", "[shutdown]".bright_red().bold());
    Ok(())
}
