use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

fn synthetic_referrer_address() -> String {
    // Any valid address works as a referrer; it does not need an associated wallet.
    // Use a deterministic non-self address for test stability.
    let addr = [9u8; 32];
    knotcoin::crypto::keys::encode_address_string(&addr)
}

fn pick_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

fn start_knotcoind(rpc_port: u16, p2p_port: u16, data_dir: &PathBuf) -> Child {
    let bin = env!("CARGO_BIN_EXE_knotcoind");

    std::fs::create_dir_all(data_dir).unwrap();

    Command::new(bin)
        .arg(format!("--rpc-port={rpc_port}"))
        .arg(format!("--p2p-port={p2p_port}"))
        .arg(format!("--data-dir={}", data_dir.to_string_lossy()))
        .env("KNOTCOIN_BOOTSTRAP_PEERS", "")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn knotcoind")
}

fn wait_for_cookie(data_dir: &PathBuf, timeout: Duration) -> String {
    let cookie_path = data_dir.join(".cookie");
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(s) = std::fs::read_to_string(&cookie_path) {
            let t = s.trim().to_string();
            if t.len() >= 32 {
                return t;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("cookie not found at {}", cookie_path.display());
}

async fn rpc_call(rpc_port: u16, token: &str, method: &str, params: Value) -> Value {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    });
    let body_bytes = serde_json::to_vec(&body).unwrap();

    let mut stream = TcpStream::connect(("127.0.0.1", rpc_port))
        .await
        .expect("connect rpc");

    let req = format!(
        "POST /rpc HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nAuthorization: Bearer {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        token,
        body_bytes.len()
    );

    stream.write_all(req.as_bytes()).await.unwrap();
    stream.write_all(&body_bytes).await.unwrap();

    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).await.unwrap();

    // Split HTTP headers
    let resp_str = String::from_utf8_lossy(&resp);
    let parts: Vec<&str> = resp_str.split("\r\n\r\n").collect();
    assert!(parts.len() >= 2, "invalid http response");
    let json_part = parts[parts.len() - 1];

    let v: Value = serde_json::from_str(json_part).expect("parse jsonrpc");
    if let Some(e) = v.get("error") {
        panic!("rpc error for {method}: {e}");
    }
    v.get("result").cloned().unwrap_or(Value::Null)
}

#[tokio::test]
async fn smoke_rpc_mainnet_features() {
    let rpc_port = pick_free_port();
    let p2p_port = pick_free_port();

    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("mainnet");

    let child = start_knotcoind(rpc_port, p2p_port, &data_dir);

    // Ensure child is killed even if the test fails.
    struct KillOnDrop(Child);
    impl Drop for KillOnDrop {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _guard = KillOnDrop(child);

    let token = wait_for_cookie(&data_dir, Duration::from_secs(20));

    // Basic connectivity
    let h = rpc_call(rpc_port, &token, "getblockcount", json!([])).await;
    assert!(h.as_u64().is_some());

    // Create two wallets: miner and referrer
    let miner_wallet = rpc_call(rpc_port, &token, "wallet_create", json!([])).await;
    let miner_mn = miner_wallet["mnemonic"].as_str().unwrap().to_string();
    let miner_addr = miner_wallet["address"].as_str().unwrap().to_string();

    let ref_addr = synthetic_referrer_address();

    // Mine 2 blocks to miner so it has funds for fees (genesis already exists)
    let _ = rpc_call(
        rpc_port,
        &token,
        "generatetoaddress",
        json!([2, miner_addr]),
    )
    .await;

    // Referral registration MUST be first outgoing tx (nonce==1).
    // This should now pass structural validation and consensus rules.
    let reg = rpc_call(
        rpc_port,
        &token,
        "wallet_register_referral",
        json!([miner_mn, ref_addr]),
    )
    .await;
    assert_eq!(reg["status"].as_str().unwrap_or(""), "referral_registered");

    // Mine 1 block to confirm mempool tx
    let _ = rpc_call(
        rpc_port,
        &token,
        "generatetoaddress",
        json!([1, miner_wallet["address"].as_str().unwrap()]),
    )
    .await;

    // Governance: send a signaling tx (to self) with governance_data
    let prop = "11".repeat(32); // 32 bytes hex
    let _ = rpc_call(
        rpc_port,
        &token,
        "wallet_send",
        json!([miner_wallet["mnemonic"].as_str().unwrap(), miner_wallet["address"].as_str().unwrap(), 0.00000001_f64, prop]),
    )
    .await;

    // Mine to confirm governance vote
    let _ = rpc_call(
        rpc_port,
        &token,
        "generatetoaddress",
        json!([1, miner_wallet["address"].as_str().unwrap()]),
    )
    .await;

    // Tally should be > 0
    let tally = rpc_call(rpc_port, &token, "getgovernancetally", json!([prop])).await;
    assert!(tally["total_weight_bps"].as_u64().unwrap_or(0) > 0);

    // Referral info endpoint should respond
    let info = rpc_call(rpc_port, &token, "getreferralinfo", json!([miner_wallet["address"].as_str().unwrap()])).await;
    assert!(info.get("privacy_code").is_some());
}
