// knotcoin-cli — Knotcoin Command Line Interface
//
// Lightweight client that sends JSON-RPC calls to the running daemon.
// Usage: knotcoin-cli <method> [params...]

use std::env;

use knotcoin::crypto::keys::{
    decode_address_string, derive_account_seed, derive_address, derive_master_seed,
    encode_address_string, generate_mnemonic,
};
use knotcoin::crypto::dilithium::PublicKey;

use colored::*;

fn print_usage() {
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
    println!(
        "{}",
        "                     - CLI Node Control -                        "
            .bright_yellow()
            .on_blue()
            .bold()
    );
    println!();
    println!(
        "{}",
        "  Usage: knotcoin-cli <command> [args...]"
            .bright_yellow()
            .bold()
    );
    println!();
    println!("{}", "  Commands:".bright_white().bold());
    println!(
        "  {} {:<38} {}",
        "❯".bright_black(),
        "createwallet".bright_green(),
        "Generate a new 12-word mnemonic wallet".white()
    );
    println!(
        "  {} {:<38} {}",
        "❯".bright_black(),
        "getblockcount".bright_green(),
        "Get current chain height".white()
    );
    println!(
        "  {} {:<38} {}",
        "❯".bright_black(),
        "getblockhash <height>".bright_green(),
        "Get block hash at height".white()
    );
    println!(
        "  {} {:<38} {}",
        "❯".bright_black(),
        "getblock <hash>".bright_green(),
        "Get full block data".white()
    );
    println!(
        "  {} {:<38} {}",
        "❯".bright_black(),
        "getbalance <address>".bright_green(),
        "Get balance (accepts KOT1 or hex)".white()
    );
    println!(
        "  {} {:<38} {}",
        "❯".bright_black(),
        "getmininginfo".bright_green(),
        "Get mining stats".white()
    );
    println!(
        "  {} {:<38} {}",
        "❯".bright_black(),
        "getmempoolinfo".bright_green(),
        "Get mempool stats".white()
    );
    println!(
        "  {} {:<38} {}",
        "❯".bright_black(),
        "generatetoaddress <n> <address>".bright_green(),
        "Mine N blocks to address".white()
    );
    println!(
        "  {} {:<38} {}",
        "❯".bright_black(),
        "addnode <ip:port>".bright_green(),
        "Add a new P2P peer".white()
    );
    println!(
        "  {} {:<38} {}",
        "❯".bright_black(),
        "stop".bright_green(),
        "Stop the daemon".white()
    );
    println!();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let method = &args[1];

    // Handle local commands first
    if method == "createwallet" {
        let mnemonic = generate_mnemonic();
        let master = derive_master_seed(&mnemonic, "");
        let account = derive_account_seed(&master, 0);

        // We derive a mock pubkey from the first 32 bytes of account seed for this version
        let mut pk_bytes = [0u8; 1952];
        pk_bytes[0..32].copy_from_slice(&account[0..32]);
        let pk = PublicKey(pk_bytes);
        let addr = derive_address(&pk);
        let kot1 = encode_address_string(&addr);

        println!("{}", "NEW KNOTCOIN WALLET CREATED".bright_green().bold());
        println!(
            "{} {}",
            "Mnemonic:".bright_yellow(),
            mnemonic.white().bold()
        );
        println!("{} {}", "Address: ".bright_yellow(), kot1.bright_white());
        println!();
        println!(
            "{}",
            "IMPORTANT: Write down your mnemonic. It is the ONLY way to recover your funds."
                .on_red()
                .white()
                .bold()
        );
        return Ok(());
    }

    let params: Vec<serde_json::Value> = args[2..]
        .iter()
        .map(|arg| {
            // Try KOT1 address decoding
            if let Ok(addr_bytes) = decode_address_string(arg) {
                return serde_json::json!(hex::encode(addr_bytes));
            }

            // If it's 64 chars, it's likely a hex address (32 bytes). Send as string.
            // Also if it starts with 0x (though not strictly required).
            if arg.len() == 64 || arg.starts_with("0x") {
                return serde_json::json!(arg);
            }

            // Try to parse as number
            if let Ok(n) = arg.parse::<u64>() {
                serde_json::json!(n)
            } else {
                serde_json::json!(arg)
            }
        })
        .collect();

    // Use a simple TCP connection + HTTP/1.1 request
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let rpc_port = std::env::var("KNOTCOIN_RPC_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(knotcoin::config::RPC_PORT);

    let addr = format!("127.0.0.1:{}", rpc_port);
    let mut stream = match TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(_) => {
            eprintln!(
                "{} cannot connect to knotcoind at {}",
                "error:".bright_red().bold(),
                addr
            );
            eprintln!(
                "Is the daemon running? Start it with: {}",
                "knotcoind".bright_yellow().bold()
            );
            std::process::exit(1);
        }
    };

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1,
    });

    let body = serde_json::to_string(&request_body)?;
    let http_request = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body,
    );

    stream.write_all(http_request.as_bytes()).await?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await?;

    let response_str = String::from_utf8_lossy(&response);

    // Parse out the JSON body from the HTTP response
    if let Some(body_start) = response_str.find("\r\n\r\n") {
        let json_body = &response_str[body_start + 4..];
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_body) {
            if let Some(result) = parsed.get("result") {
                println!("{}", serde_json::to_string_pretty(result)?.bright_white());
            } else if let Some(error) = parsed.get("error") {
                eprintln!(
                    "{} {}",
                    "Error:".bright_red().bold(),
                    serde_json::to_string_pretty(error)?
                );
            }
        } else {
            println!("{}", json_body);
        }
    }

    Ok(())
}
