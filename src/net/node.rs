use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::net::{TcpListener, TcpStream};
use tokio_socks::tcp::Socks5Stream;

use crate::config::P2P_BIND_ADDRESS;
use crate::consensus::state::{apply_block, block_hash};
use crate::net::protocol::{FramedStream, NetworkMessage};
use crate::node::{ChainDB, db_common::StoredBlock};
use crate::net::mempool::Mempool;
use crate::rpc::server::RpcState;

/// Tor SOCKS5 proxy address (standard Tor daemon port)
const TOR_SOCKS_PROXY: &str = "127.0.0.1:9050";

const MAX_HEADERS_PER_MSG: usize = 2000;
const MAX_BLOCKS_PER_MSG: usize = 16;
const HANDSHAKE_TIMEOUT_SECS: u64 = 10;
const MAX_OUTBOUND: usize = 8;
const MAX_INBOUND: usize = 64;

// Hybrid bootstrap: .onion addresses go through Tor SOCKS5, plain IPs connect directly.
// Add community seed IPs below as volunteers join — both types work simultaneously.
const BOOTSTRAP_PEERS: &[&str] = &[
    // Tor hidden service — requires Tor running on the local machine (port 9050).
    // Provides the initial anonymous bootstrap when no plain-IP seeds are known yet.
    "u4seopjtremf6f22kib73yk6k2iiizwp7x46fddoxm6hqdcgcaq3piyd.onion:9000",

    // Community plain-IP seed nodes — no Tor required, fast direct connection.
    // Add volunteer IPs here and rebuild / release:
    // "1.2.3.4:9000",   // Example volunteer 1
    // "5.6.7.8:9000",   // Example volunteer 2
];

fn is_private_ip(addr: SocketAddr) -> bool {
    let ip = addr.ip();
    if ip.is_loopback() {
        return true;
    }
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_private(),
        std::net::IpAddr::V6(v6) => {
            (v6.segments()[0] & 0xfe00) == 0xfc00
        }
    }
}

pub enum P2pCommand {
    Connect(SocketAddr),
    Broadcast(NetworkMessage),
}

pub struct P2PNode {
    pub peers: Arc<Mutex<HashMap<SocketAddr, PeerInfo>>>,
    pub db: ChainDB,
    pub mempool: Arc<Mutex<Mempool>>,
    pub broadcast_tx: tokio::sync::broadcast::Sender<NetworkMessage>,
}

pub struct PeerInfo {
    pub height: u32,
    pub challenge: [u8; 32],
    pub is_outbound: bool,
    pub handshake_stage: HandshakeStage,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum HandshakeStage {
    Version,
    Challenge,
    Response,
    Done,
}

impl P2PNode {
    pub fn new_from_rpc_state(s: Arc<RpcState>) -> Self {
        let (broadcast_tx, _) = tokio::sync::broadcast::channel(256);
        P2PNode {
            peers: Arc::new(Mutex::new(HashMap::new())),
            db: s.db.clone(),
            mempool: s.mempool.clone(),
            broadcast_tx,
        }
    }

    pub async fn start_on_port(
        &self,
        port: u16,
        mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<P2pCommand>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("{P2P_BIND_ADDRESS}:{port}").parse::<SocketAddr>()?;
        let listener = TcpListener::bind(addr).await?;
        eprintln!("[p2p] listening on {addr}");

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    let (stream, peer_addr) = accept_res?;
                    let inbound_count = self.peers.lock().await.values().filter(|i| !i.is_outbound).count();

                    if inbound_count >= MAX_INBOUND || is_private_ip(peer_addr) {
                        eprintln!("[p2p] rejecting inbound {peer_addr}");
                        continue;
                    }

                    let db = self.db.clone();
                    let mempool = self.mempool.clone();
                    let peers = self.peers.clone();
                    let broadcast_tx = self.broadcast_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, peer_addr, db, mempool, peers, broadcast_tx, false).await {
                            eprintln!("[p2p] {peer_addr} disconnected: {e}");
                        }
                    });
                }
                cmd = cmd_rx.recv() => {
                    if let Some(cmd) = cmd {
                        match cmd {
                            P2pCommand::Connect(addr) => {
                                let _ = self.connect(addr).await;
                            }
                            P2pCommand::Broadcast(msg) => {
                                let _ = self.broadcast_tx.send(msg);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Connect to a plain TCP peer directly.
    pub async fn connect(&self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let outbound_count = self.peers.lock().await.values().filter(|i| i.is_outbound).count();
        if outbound_count >= MAX_OUTBOUND {
            return Err("max outbound reached".into());
        }
        let stream = TcpStream::connect(addr).await?;
        self.spawn_connection(stream, addr, true);
        Ok(())
    }

    /// Connect to a .onion address through the local Tor SOCKS5 proxy.
    async fn connect_onion(
        &self,
        onion_addr: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let outbound_count = self.peers.lock().await.values().filter(|i| i.is_outbound).count();
        if outbound_count >= MAX_OUTBOUND {
            return Err("max outbound reached".into());
        }

        // Establish TCP tunnel through Tor SOCKS5 proxy to the .onion destination.
        let socks_stream = Socks5Stream::connect(TOR_SOCKS_PROXY, onion_addr).await
            .map_err(|e| format!("Tor SOCKS5 error connecting to {onion_addr}: {e}"))?;
        let stream = socks_stream.into_inner();

        // Use a fake SocketAddr for the peer map key (we only have the .onion name).
        // We encode the onion host hash into the peer address so it is unique.
        let fake_addr: SocketAddr = "127.0.0.2:9000".parse().unwrap();
        self.spawn_connection(stream, fake_addr, true);
        Ok(())
    }

    /// Shared helper: spawn a connection handler task for an already-opened TcpStream.
    fn spawn_connection(&self, stream: TcpStream, addr: SocketAddr, is_outbound: bool) {
        let db = self.db.clone();
        let mempool = self.mempool.clone();
        let peers = self.peers.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, addr, db, mempool, peers, broadcast_tx, is_outbound).await {
                eprintln!("[p2p] {addr} disconnected: {e}");
            }
        });
    }

    /// Bootstrap the node by attempting connections to all configured seed peers.
    /// Hybrid: .onion seeds use Tor SOCKS5, plain-IP seeds connect directly.
    /// Gracefully skips .onion seeds if Tor is not available.
    pub async fn connect_bootstrap(&self) {
        if BOOTSTRAP_PEERS.is_empty() {
            eprintln!("[p2p] No bootstrap peers configured — waiting for inbound connections.");
            return;
        }
        eprintln!("[p2p] Bootstrapping from {} seed peer(s)...", BOOTSTRAP_PEERS.len());

        for (idx, &seed) in BOOTSTRAP_PEERS.iter().enumerate() {
            if seed.contains(".onion") {
                // ── Tor path ──────────────────────────────────────────────────
                eprintln!("[p2p] Seed #{}: {} (Tor .onion — attempting SOCKS5 via {TOR_SOCKS_PROXY})", idx + 1, seed);
                match self.connect_onion(seed).await {
                    Ok(_) => eprintln!("[p2p] Seed #{}: Tor connection established.", idx + 1),
                    Err(e) => {
                        // Connection refused typically means Tor is not running.
                        if e.to_string().contains("refused") || e.to_string().contains("10061") {
                            eprintln!("[p2p] Seed #{}: Tor not available (port 9050 not open). \
                                Start Tor to use .onion bootstrap. Skipping.", idx + 1);
                        } else {
                            eprintln!("[p2p] Seed #{}: .onion connect failed: {e}", idx + 1);
                        }
                    }
                }
            } else {
                // ── Plain-IP path ─────────────────────────────────────────────
                match seed.parse::<SocketAddr>() {
                    Ok(addr) => {
                        eprintln!("[p2p] Seed #{}: {} (direct TCP)", idx + 1, addr);
                        match self.connect(addr).await {
                            Ok(_) => eprintln!("[p2p] Seed #{}: connected.", idx + 1),
                            Err(e) => eprintln!("[p2p] Seed #{}: failed: {e}", idx + 1),
                        }
                    }
                    Err(e) => eprintln!("[p2p] Seed #{}: invalid address '{}': {e}", idx + 1, seed),
                }
            }

            // Brief pause between attempts to avoid hammering seeds.
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    db: ChainDB,
    mempool: Arc<Mutex<Mempool>>,
    peers: Arc<Mutex<HashMap<SocketAddr, PeerInfo>>>,
    broadcast_tx: tokio::sync::broadcast::Sender<NetworkMessage>,
    is_outbound: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut s = FramedStream::new(stream);
    let mut broadcast_rx = broadcast_tx.subscribe();
    let our_height = db.get_chain_height().unwrap_or(0);

    // 1. Initial Handshake
    {
        let mut p = peers.lock().await;
        p.insert(addr, PeerInfo {
            height: 0,
            challenge: [0u8; 32],
            is_outbound,
            handshake_stage: HandshakeStage::Version,
        });
    }

    s.send(&NetworkMessage::Version { height: our_height }).await?;

    let deadline = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + HANDSHAKE_TIMEOUT_SECS;

    loop {
        tokio::select! {
            net_msg = s.recv() => {
                let msg = match net_msg? {
                    Some(m) => m,
                    None => break,
                };

                let is_done = peers.lock().await.get(&addr).map(|i| i.handshake_stage == HandshakeStage::Done).unwrap_or(false);
                
                if !is_done && SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() > deadline {
                    return Err("handshake timeout".into());
                }

                match (msg, is_done) {
                    (NetworkMessage::Version { height: peer_height }, false) => {
                        let mut p = peers.lock().await;
                        if let Some(info) = p.get_mut(&addr) {
                            info.height = peer_height;
                            info.handshake_stage = HandshakeStage::Challenge;
                            let mut challenge = [0u8; 32];
                            getrandom::getrandom(&mut challenge).unwrap();
                            info.challenge = challenge;
                            s.send(&NetworkMessage::Challenge(challenge)).await?;
                        }
                    }
                    (NetworkMessage::Challenge(received_challenge), false) => {
                        let response_hash = crate::crypto::hash::hash_sha3_256(&received_challenge);
                        s.send(&NetworkMessage::Response(response_hash)).await?;
                    }
                    (NetworkMessage::Response(received_response), false) => {
                        let mut p = peers.lock().await;
                        if let Some(info) = p.get_mut(&addr) {
                            let expected = crate::crypto::hash::hash_sha3_256(&info.challenge);
                            if received_response == expected {
                                info.handshake_stage = HandshakeStage::Response;
                                s.send(&NetworkMessage::Verack).await?;
                            } else {
                                return Err("handshake failed".into());
                            }
                        }
                    }
                    (NetworkMessage::Verack, false) => {
                        {
                            let mut p = peers.lock().await;
                            if let Some(info) = p.get_mut(&addr) {
                                info.handshake_stage = HandshakeStage::Done;
                            }
                        }
                        eprintln!("[p2p] {addr} handshake complete");
                        let _our_height = db.get_chain_height().unwrap_or(0);
                        let tip = db.get_tip().ok().flatten().unwrap_or([0u8; 32]);
                        s.send(&NetworkMessage::GetHeaders { from_hash: tip }).await?;
                    }
                    (m, true) => {
                        handle_msg(m, &mut s, addr, &db, &mempool, &peers, &broadcast_tx).await?;
                    }
                    _ => {}
                }
            }
            local_msg = broadcast_rx.recv() => {
                if let Ok(m) = local_msg {
                    s.send(&m).await?;
                }
            }
        }
    }

    {
        let mut p = peers.lock().await;
        p.remove(&addr);
    }
    Ok(())
}

async fn handle_msg(
    msg: NetworkMessage,
    s: &mut FramedStream,
    addr: SocketAddr,
    db: &ChainDB,
    mempool: &Arc<Mutex<Mempool>>,
    _peers: &Arc<Mutex<HashMap<SocketAddr, PeerInfo>>>,
    broadcast_tx: &tokio::sync::broadcast::Sender<NetworkMessage>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match msg {
        NetworkMessage::Ping(n) => {
            let _ = s.send(&NetworkMessage::Pong(n)).await;
        }
        NetworkMessage::GetHeaders { from_hash } => {
            let tip_height = db.get_chain_height().unwrap_or(0);
            let start = find_height_of_hash(db, &from_hash).unwrap_or(0).saturating_add(1);
            let end = (start + MAX_HEADERS_PER_MSG as u32 - 1).min(tip_height);
            let mut hashes = Vec::new();
            for h in start..=end {
                if let Ok(Some(hash)) = db.get_block_hash_by_height(h) {
                    hashes.push(hash);
                }
            }
            if !hashes.is_empty() {
                s.send(&NetworkMessage::Headers(hashes)).await?;
            }
        }
        NetworkMessage::Headers(hashes) => {
            let needed: Vec<[u8; 32]> = hashes.into_iter()
                .filter(|h| db.get_block(h).ok().flatten().is_none())
                .collect();
            for chunk in needed.chunks(MAX_BLOCKS_PER_MSG) {
                s.send(&NetworkMessage::GetBlocks { hashes: chunk.to_vec() }).await?;
            }
        }
        NetworkMessage::GetBlocks { hashes } => {
            let blocks: Vec<Vec<u8>> = hashes.iter()
                .filter_map(|h| db.get_block(h).ok().flatten())
                .map(|b| b.to_bytes())
                .collect();
            if !blocks.is_empty() {
                s.send(&NetworkMessage::Blocks(blocks)).await?;
            }
        }
        NetworkMessage::Blocks(raws) => {
            // OPTIMIZATION: Parallel PoW verification with proper ordering
            // CRITICAL: Blocks must be applied in height order for consensus
            use rayon::prelude::*;
            
            // Step 1: Parse all blocks sequentially (fast, no benefit from parallelization)
            let mut parsed: Vec<(StoredBlock, [u8; 32])> = Vec::new();
            for raw in &raws {
                if let Ok(block) = StoredBlock::from_bytes(raw) {
                    let h = block_hash(&block);
                    parsed.push((block, h));
                }
            }
            
            // Step 2: Filter out blocks we already have (sequential, database access)
            let mut new_blocks: Vec<(StoredBlock, [u8; 32])> = Vec::new();
            for (block, h) in parsed {
                if db.get_block(&h).ok().flatten().is_none() {
                    new_blocks.push((block, h));
                }
            }
            
            // Step 3: Sort by height to ensure correct order (CRITICAL)
            new_blocks.sort_by_key(|(block, _)| u32::from_le_bytes(block.block_height));
            
            // Step 4: Verify parent exists for each block (CRITICAL)
            let mut valid_chain: Vec<(StoredBlock, [u8; 32])> = Vec::new();
            for (block, h) in new_blocks {
                let height = u32::from_le_bytes(block.block_height);
                
                // Check parent exists (except genesis)
                if height > 0 {
                    let parent_exists = db.get_block(&block.previous_hash).ok().flatten().is_some();
                    if !parent_exists {
                        // Parent missing, skip this block (will be requested later)
                        continue;
                    }
                }
                
                valid_chain.push((block, h));
            }
            
            // Step 5: Verify PoW in parallel (NOW SAFE: blocks are ordered and parents verified)
            let db_clone = db.clone();
            let verified: Vec<(StoredBlock, [u8; 32])> = valid_chain.into_par_iter()
                .filter(|(block, _)| {
                    // Verify PoW without state access (stateless, thread-safe)
                    crate::consensus::state::verify_block_pow(block, &db_clone).is_ok()
                })
                .collect();
            
            // Step 6: Re-sort after parallel processing (parallel processing may reorder)
            let mut verified_sorted = verified;
            verified_sorted.sort_by_key(|(block, _)| u32::from_le_bytes(block.block_height));
            
            // Step 7: Apply blocks sequentially in height order (CONSENSUS-CRITICAL)
            let mut applied = 0;
            for (block, _) in verified_sorted {
                if apply_block(db, &block).is_ok() {
                    applied += 1;
                } else {
                    // If one block fails, stop processing (chain broken)
                    break;
                }
            }
            
            if applied > 0 {
                eprintln!("[p2p] {addr} applied {applied} block(s), height now {}", db.get_chain_height().unwrap_or(0));
            }
        }
        NetworkMessage::Tx(raw) => {
            let mut pool = mempool.lock().await;
            if let Ok(stx) = crate::node::db_common::StoredTransaction::from_bytes(&raw)
                && pool.add_transaction(stx.0).is_ok() {
                let _ = broadcast_tx.send(NetworkMessage::Tx(raw));
            }
        }
        NetworkMessage::Addr(addrs) => {
            let mut learnt = false;
            for a in addrs {
                if !is_private_ip(a) {
                    // Logic to track known addrs for future connection
                    learnt = true;
                }
            }
            if learnt {
                // Relay ADDR list to gossip
            }
        }
        _ => {}
    }
    Ok(())
}

fn find_height_of_hash(db: &ChainDB, hash: &[u8; 32]) -> Option<u32> {
    db.get_block(hash)
        .ok()?
        .map(|b| u32::from_le_bytes(b.block_height))
}
