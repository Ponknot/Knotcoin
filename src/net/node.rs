use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::PathBuf;
use std::fs;
use serde_json;

use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use crate::config::{default_data_dir, p2p_bind_address};
use crate::consensus::state::{apply_block, block_hash};
use crate::net::protocol::{FramedStream, NetworkMessage};
use crate::node::{ChainDB, db_common::StoredBlock};
use crate::net::mempool::Mempool;
use crate::rpc::server::RpcState;

const MAX_INBOUND: usize = 128; // Increased to allow seed nodes to accept more peers
const MAX_OUTBOUND: usize = 32;
const HANDSHAKE_TIMEOUT_SECS: u64 = 10;
const MAX_HEADERS_PER_MSG: usize = 500;
const MAX_BLOCKS_PER_MSG: usize = 50;
const OUTBOUND_CONNECT_TIMEOUT_SECS: u64 = 3;

/// Bootstrap seed nodes with automatic phase-out based on blockchain height
/// Can be overridden with KNOTCOIN_BOOTSTRAP_PEERS environment variable
const BOOTSTRAP_SEEDS_PHASE1: &[&str] = &[
    "seed.knotcoin.network:9000",           // DNS seed (recommended)
    "104.229.254.145:9000",                 // Community seed node (volunteer)
];

const PERMANENT_SEEDS: &[&str] = &[
    "seed.knotcoin.network:9000",           // DNS seed (recommended)
    "104.229.254.145:9000",                 // Community seed node (volunteer)
];

/// Load bootstrap peers from environment variable or use defaults
fn load_bootstrap_peers() -> Vec<String> {
    if let Ok(peers_str) = std::env::var("KNOTCOIN_BOOTSTRAP_PEERS") {
        peers_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new() // Return empty if env var not set
    }
}

fn dev_allow_local() -> bool {
    std::env::var("KNOTCOIN_DEV_ALLOW_LOCAL")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Optional advertised addresses for bootstrap nodes.
/// Comma-separated list of socket addresses, e.g. "203.0.113.5:9000,[::1]:9000"
fn load_advertised_addrs() -> Vec<SocketAddr> {
    if let Ok(addrs_str) = std::env::var("KNOTCOIN_ADVERTISE_ADDRS") {
        addrs_str
            .split(',')
            .filter_map(|s| s.trim().parse::<SocketAddr>().ok())
            .collect()
    } else {
        Vec::new()
    }
}

/// Load optional seed list from a text file (one host:port per line, # comments)
fn load_seedlist_file(path: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        out.push(trimmed.to_string());
    }
    out
}

fn load_seedlist() -> Vec<String> {
    if let Ok(path) = std::env::var("KNOTCOIN_SEEDLIST") {
        let p = std::path::PathBuf::from(path);
        let list = load_seedlist_file(&p);
        if !list.is_empty() {
            return list;
        }
    }
    let path = data_dir_path().join("seedlist.txt");
    ensure_seedlist_template(&path);
    load_seedlist_file(&path)
}

fn ensure_seedlist_template(path: &std::path::Path) {
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let template = [
        "# Knotcoin seed list",
        "# One host:port per line. Lines starting with # are ignored.",
        "# Add community-run public nodes here to improve bootstrap reliability.",
        "# Example:",
        "# 203.0.113.10:9000",
        "",
    ].join("\n");
    let _ = fs::write(path, template);
}

/// Select seed nodes based on blockchain height
fn get_bootstrap_peers(current_height: u32) -> Vec<String> {
    // Priority 1: Environment variable (for privacy)
    let env_peers = load_bootstrap_peers();
    if !env_peers.is_empty() {
        return env_peers;
    }
    
    // Priority 2: Optional seed list file (user/community provided)
    let mut peers = load_seedlist();

    // Priority 3: Default seeds based on network maturity
    if current_height < 5000 {
        peers.extend(BOOTSTRAP_SEEDS_PHASE1.iter().map(|s| s.to_string()));
    } else {
        peers.extend(PERMANENT_SEEDS.iter().map(|s| s.to_string()));
    }

    // Priority 4: Previously known peers (best-effort fallback)
    for a in load_known_peers() {
        peers.push(a.to_string());
    }

    // Randomize to avoid a fixed order
    if peers.len() > 1 {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        peers.shuffle(&mut rng);
    }

    peers
}

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

#[derive(Clone)]
pub struct P2PNode {
    pub peers: Arc<Mutex<HashMap<SocketAddr, PeerInfo>>>,
    pub known_addrs: Arc<Mutex<HashSet<SocketAddr>>>,
    pub db: ChainDB,
    pub mempool: Arc<Mutex<Mempool>>,
    pub broadcast_tx: tokio::sync::broadcast::Sender<NetworkMessage>,
    pub connected_peers: Arc<std::sync::atomic::AtomicUsize>,
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
        let mut known = load_known_peers();
        let advertised = load_advertised_addrs();
        if !advertised.is_empty() {
            for a in advertised {
                if dev_allow_local() || !is_private_ip(a) {
                    known.insert(a);
                }
            }
        }
        P2PNode {
            peers: Arc::new(Mutex::new(HashMap::new())),
            known_addrs: Arc::new(Mutex::new(known)),
            db: s.db.clone(),
            mempool: s.mempool.clone(),
            broadcast_tx,
            connected_peers: s.connected_peers.clone(),
        }
    }

    /// Shared helper: spawn a connection handler task for an already-opened TcpStream.
    fn spawn_connection(&self, stream: TcpStream, addr: SocketAddr, is_outbound: bool) {
        let db = self.db.clone();
        let mempool = self.mempool.clone();
        let peers = self.peers.clone();
        let known_addrs = self.known_addrs.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, addr, db, mempool, peers, known_addrs, broadcast_tx, is_outbound).await {
                println!("[p2p] {addr} disconnected: {e}");
            }
        });
    }

    pub async fn start_on_port(
        &self,
        port: u16,
        mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<P2pCommand>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("{}:{port}", p2p_bind_address()).parse::<SocketAddr>()?;
        
        let socket = socket2::Socket::new(
            match addr {
                SocketAddr::V4(_) => socket2::Domain::IPV4,
                SocketAddr::V6(_) => socket2::Domain::IPV6,
            },
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )?;

        // Enable port and address reuse to prevent "os error 10048" on quick restarts
        socket.set_reuse_address(true)?;
        #[cfg(all(unix, not(target_os = "solaris"), not(target_os = "illumos")))]
        socket.set_reuse_port(true)?;

        socket.bind(&addr.into())?;
        socket.listen(1024)?;

        let std_listener: std::net::TcpListener = socket.into();
        std_listener.set_nonblocking(true)?;
        let listener = TcpListener::from_std(std_listener)?;
        
        println!("[p2p] listening on {addr}");
        
        // Spawn the lightweight peer count sync loop
        let cp = self.connected_peers.clone();
        let p_map = self.peers.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                cp.store(p_map.lock().await.len(), std::sync::atomic::Ordering::Relaxed);
            }
        });

        // Spawn the known-peer dialer loop (gradually forms a mesh beyond the seed).
        // Tries a few known peers periodically when outbound slots are available.
        let dialer = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

                let outbound_count = dialer.peers.lock().await.values().filter(|i| i.is_outbound).count();
                if outbound_count >= MAX_OUTBOUND {
                    continue;
                }

                // Pick up to 2 candidates we are not already connected to.
                let connected: HashSet<SocketAddr> = dialer.peers.lock().await.keys().cloned().collect();
                let candidates: Vec<SocketAddr> = {
                    let known = dialer.known_addrs.lock().await;
                    known
                        .iter()
                        .cloned()
                        .filter(|a| !connected.contains(a))
                        .take(2)
                        .collect()
                };

                for addr in candidates {
                    let _ = dialer.connect(addr).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }
            }
        });

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    let (stream, peer_addr) = accept_res?;
                    let inbound_count = self.peers.lock().await.values().filter(|i| !i.is_outbound).count();

                    if inbound_count >= MAX_INBOUND || (!dev_allow_local() && is_private_ip(peer_addr)) {
                        println!("[p2p] rejecting inbound {peer_addr}");
                        continue;
                    }

                    self.spawn_connection(stream, peer_addr, false);
                }
                cmd = cmd_rx.recv() => {
                    if let Some(cmd) = cmd {
                        match cmd {
                            P2pCommand::Connect(addr) => {
                                // Run outbound dials in the background so we don't stall accept/broadcast loops.
                                let node = self.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = node.connect(addr).await {
                                        println!("[p2p] ✗ dial {addr} failed: {e}");
                                    }
                                });
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
        if !dev_allow_local() && is_private_ip(addr) {
            return Err("refusing private/loopback peer (set KNOTCOIN_DEV_ALLOW_LOCAL=1 for local testing)".into());
        }
        let outbound_count = self.peers.lock().await.values().filter(|i| i.is_outbound).count();
        if outbound_count >= MAX_OUTBOUND {
            return Err("max outbound reached".into());
        }

        // Remember the peer for future runs and make the behavior visible in logs.
        {
            let mut known = self.known_addrs.lock().await;
            known.insert(addr);
        }
        save_known_peers(&self.known_addrs).await;

        println!("[p2p] → dialing {addr}");
        let stream = timeout(
            tokio::time::Duration::from_secs(OUTBOUND_CONNECT_TIMEOUT_SECS),
            TcpStream::connect(addr)
        ).await??;

        self.spawn_connection(stream, addr, true);
        
        Ok(())
    }

    /// Bootstrap the node by attempting connections to configured seed peers.
    /// Connects directly to known IP seeds.
    pub async fn connect_bootstrap(&self) {
        // Get current blockchain height for smart seed selection
        let current_height = match self.db.get_chain_height() {
            Ok(h) => h,
            Err(_) => 0
        };
        
        // Get appropriate seeds based on network maturity (silent phase transition)
        let bootstrap_peers = get_bootstrap_peers(current_height);
        
        if bootstrap_peers.is_empty() {
            return;
        }

        let mut connected_count = 0u32;

        for (idx, seed) in bootstrap_peers.iter().enumerate() {
            let mut addrs: Vec<SocketAddr> = Vec::new();

            if let Ok(addr) = seed.parse::<SocketAddr>() {
                addrs.push(addr);
            } else if let Ok(resolved) = tokio::net::lookup_host(seed).await {
                addrs.extend(resolved);
            }

            if addrs.is_empty() {
                println!("[p2p] Seed #{}: could not resolve {}", idx + 1, seed);
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                continue;
            }

            for addr in addrs {
                // Remember the seed for future runs.
                {
                    let mut known = self.known_addrs.lock().await;
                    known.insert(addr);
                }
                match self.connect(addr).await {
                    Ok(_) => {
                        println!("[p2p] ✓ Seed #{}: connected to {}", idx + 1, addr);
                        connected_count += 1;
                        break; // stop after first successful address for this seed
                    }
                    Err(e) => {
                        if !e.to_string().contains("refused") && !e.to_string().contains("10061") {
                            println!("[p2p] Seed #{}: {e}", idx + 1);
                        }
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        if connected_count > 0 {
            println!("[p2p] bootstrap complete: {} seed(s) connected", connected_count);
        } else {
            println!("[p2p] ⚠ bootstrap: no seeds reachable (check Tor/network)");
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    db: ChainDB,
    mempool: Arc<Mutex<Mempool>>,
    peers: Arc<Mutex<HashMap<SocketAddr, PeerInfo>>>,
    known_addrs: Arc<Mutex<HashSet<SocketAddr>>>,
    broadcast_tx: tokio::sync::broadcast::Sender<NetworkMessage>,
    is_outbound: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut s = FramedStream::new(stream);
    let mut broadcast_rx = broadcast_tx.subscribe();
    let our_height = db.get_chain_height().unwrap_or(0);

    if is_outbound {
        println!("[p2p] handshake start (outbound) {addr}");
    } else {
        println!("[p2p] handshake start (inbound) {addr}");
    }

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
                        
                        let our_height = db.get_chain_height().unwrap_or(0);
                        let peer_height = peers.lock().await.get(&addr).map(|i| i.height).unwrap_or(0);
                        
                        if peer_height > our_height {
                            println!("[p2p] ✓ {addr} connected (peer: {peer_height}, us: {our_height}) - syncing...");
                        } else {
                            println!("[p2p] ✓ {addr} connected (peer: {peer_height}, us: {our_height})");
                        }
                        
                        // Start sync from our current tip
                        let tip = db.get_tip().ok().flatten().unwrap_or([0u8; 32]);
                        s.send(&NetworkMessage::GetHeaders { from_hash: tip }).await?;

                        // Peer discovery: send a small list of known peers after handshake.
                        // This helps form a mesh and reduces dependency on bootstrap seeds.
                        let mut list: Vec<SocketAddr> = {
                            let known = known_addrs.lock().await;
                            known.iter().cloned().filter(|a| *a != addr).take(32).collect()
                        };
                        // Also include any currently connected peers (excluding the recipient).
                        let connected_peers: Vec<SocketAddr> = peers.lock().await.keys().cloned().filter(|a| *a != addr).take(32).collect();
                        list.extend(connected_peers);
                        list.sort();
                        list.dedup();
                        if !list.is_empty() {
                            let _ = s.send(&NetworkMessage::Addr(list)).await;
                        }
                        
                        // Request peers from the connected node (Bitcoin-style peer discovery)
                        let _ = s.send(&NetworkMessage::GetAddr).await;
                    }
                    (m, true) => {
                        handle_msg(m, &mut s, addr, &db, &mempool, &peers, &known_addrs, &broadcast_tx).await?;
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
    known_addrs: &Arc<Mutex<HashSet<SocketAddr>>>,
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
            if hashes.is_empty() {
                // No more headers - we're synced!
                let our_height = db.get_chain_height().unwrap_or(0);
                println!("[p2p] ✓ {addr} sync complete at height {our_height}");
                return Ok(());
            }
            
            // Filter blocks we don't have yet
            let needed: Vec<[u8; 32]> = hashes.into_iter()
                .filter(|h| db.get_block(h).ok().flatten().is_none())
                .collect();
            
            if needed.is_empty() {
                // We have all these blocks, continue syncing
                let tip = db.get_tip().ok().flatten().unwrap_or([0u8; 32]);
                s.send(&NetworkMessage::GetHeaders { from_hash: tip }).await?;
                return Ok(());
            }
            
            println!("[p2p] ← {addr} requesting {} block(s)...", needed.len());
            
            // Request blocks in chunks for smooth download
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
            // OPTIMIZATION: Fast, smooth, error-free block sync
            // Design: Parallel PoW verification + Sequential consensus application
            use rayon::prelude::*;
            
            if raws.is_empty() {
                return Ok(());
            }
            
            // Step 1: Parse all blocks (fast, sequential)
            let mut parsed: Vec<(StoredBlock, [u8; 32])> = Vec::new();
            for raw in &raws {
                match StoredBlock::from_bytes(raw) {
                    Ok(block) => {
                        let h = block_hash(&block);
                        parsed.push((block, h));
                    }
                    Err(e) => {
                        eprintln!("[p2p] {addr} sent malformed block: {e}");
                        continue; // Skip bad blocks, don't disconnect
                    }
                }
            }
            
            if parsed.is_empty() {
                return Ok(());
            }
            
            // Step 2: Filter out blocks we already have
            let mut new_blocks: Vec<(StoredBlock, [u8; 32])> = Vec::new();
            for (block, h) in parsed {
                match db.get_block(&h) {
                    Ok(Some(_)) => continue, // Already have it
                    Ok(None) => new_blocks.push((block, h)),
                    Err(e) => {
                        println!("[p2p] database error checking block: {e}");
                        continue;
                    }
                }
            }
            
            if new_blocks.is_empty() {
                return Ok(());
            }
            
            // Step 3: Sort by height (CRITICAL for consensus)
            new_blocks.sort_by_key(|(block, _)| u32::from_le_bytes(block.block_height));
            
            // Step 4: Verify parent chain exists
            let mut valid_chain: Vec<(StoredBlock, [u8; 32])> = Vec::new();
            for (block, h) in new_blocks {
                let height = u32::from_le_bytes(block.block_height);
                
                // Genesis block has no parent
                if height == 0 {
                    valid_chain.push((block, h));
                    continue;
                }
                
                // Check parent exists
                match db.get_block(&block.previous_hash) {
                    Ok(Some(_)) => {
                        valid_chain.push((block, h));
                    }
                    Ok(None) => {
                        // Parent missing - request it
                        eprintln!("[p2p] {addr} block {} missing parent, requesting...", height);
                        let _ = s.send(&NetworkMessage::GetBlocks { 
                            hashes: vec![block.previous_hash] 
                        }).await;
                        // Don't process this block yet
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[p2p] database error checking parent: {e}");
                        continue;
                    }
                }
            }
            
            if valid_chain.is_empty() {
                return Ok(());
            }
            
            // Step 5: Parallel PoW verification (FAST)
            // This is the bottleneck - use all CPU cores
            let db_clone = db.clone();
            let verified: Vec<(StoredBlock, [u8; 32])> = valid_chain.into_par_iter()
                .filter_map(|(block, h)| {
                    match crate::consensus::state::verify_block_pow(&block, &db_clone) {
                        Ok(_) => Some((block, h)),
                        Err(e) => {
                            let height = u32::from_le_bytes(block.block_height);
                            eprintln!("[p2p] {addr} block {} failed PoW: {e}", height);
                            None
                        }
                    }
                })
                .collect();
            
            if verified.is_empty() {
                eprintln!("[p2p] {addr} sent blocks with invalid PoW");
                return Ok(());
            }
            
            // Step 6: Re-sort after parallel processing
            let mut verified_sorted = verified;
            verified_sorted.sort_by_key(|(block, _)| u32::from_le_bytes(block.block_height));
            
            // Step 7: Apply blocks sequentially (CONSENSUS-CRITICAL)
            let mut applied = 0;
            let mut failed = 0;
            for (block, _hash) in verified_sorted {
                let height = u32::from_le_bytes(block.block_height);
                
                match apply_block(db, &block) {
                    Ok(_) => {
                        applied += 1;
                    }
                    Err(e) => {
                        println!("[p2p] {addr} block {} apply failed: {e}", height);
                        failed += 1;
                        // Stop processing on first failure (chain broken)
                        break;
                    }
                }
            }
            
            if applied > 0 {
                let new_height = db.get_chain_height().unwrap_or(0);
                println!("[p2p] ✓ {addr} synced +{applied} blocks → height {new_height}");
                
                // Continue syncing if we got a full batch
                if applied >= MAX_BLOCKS_PER_MSG {
                    let tip = db.get_tip().ok().flatten().unwrap_or([0u8; 32]);
                    let _ = s.send(&NetworkMessage::GetHeaders { from_hash: tip }).await;
                }
            }
            
            if failed > 0 {
                println!("[p2p] ✗ {addr} sync stopped: {failed} block(s) failed validation");
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
            let mut newly_learned: Vec<SocketAddr> = Vec::new();
            {
                let mut known = known_addrs.lock().await;
                for a in addrs {
                    if a == addr {
                        continue;
                    }
                    if !dev_allow_local() && is_private_ip(a) {
                        continue;
                    }
                    // Hard cap known peers to avoid unbounded growth.
                    if known.len() >= 2048 {
                        break;
                    }
                    if known.insert(a) {
                        newly_learned.push(a);
                    }
                }
            }

            if !newly_learned.is_empty() {
                save_known_peers(known_addrs).await;

                // Gossip the newly learned addresses (bounded) to other peers.
                newly_learned.sort();
                newly_learned.truncate(64);
                let _ = broadcast_tx.send(NetworkMessage::Addr(newly_learned));
            }
        }
        NetworkMessage::GetAddr => {
            // Respond with our known peers (up to 64)
            let list: Vec<SocketAddr> = {
                let known = known_addrs.lock().await;
                known.iter().cloned().filter(|a| *a != addr).take(64).collect()
            };
            if !list.is_empty() {
                let _ = s.send(&NetworkMessage::Addr(list)).await;
            }
        }
        _ => {}
    }
    Ok(())
}

fn data_dir_path() -> PathBuf {
    if let Ok(d) = std::env::var("KNOTCOIN_DATA_DIR") {
        return PathBuf::from(d);
    }
    default_data_dir()
}

fn known_peers_file() -> PathBuf {
    data_dir_path().join("peers.json")
}

fn load_known_peers() -> HashSet<SocketAddr> {
    let path = known_peers_file();
    let mut out = HashSet::new();
    if let Ok(s) = fs::read_to_string(&path) {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(&s) {
            for item in list {
                if let Ok(a) = item.parse::<SocketAddr>() {
                    if dev_allow_local() || !is_private_ip(a) {
                        out.insert(a);
                    }
                }
            }
        }
    }
    out
}

async fn save_known_peers(known_addrs: &Arc<Mutex<HashSet<SocketAddr>>>) {
    let path = known_peers_file();
    let list: Vec<String> = {
        let known = known_addrs.lock().await;
        known.iter().take(2048).map(|a| a.to_string()).collect()
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string(&list) {
        let _ = fs::write(path, data);
    }
}

fn find_height_of_hash(db: &ChainDB, hash: &[u8; 32]) -> Option<u32> {
    db.get_block(hash)
        .ok()?
        .map(|b| u32::from_le_bytes(b.block_height))
}
