// Shared database types used by both sled and RocksDB implementations
// These types define the on-disk format and must remain stable

use serde::{Deserialize, Serialize};
use crate::crypto::keys::ADDRESS_BYTES;

/// Account state stored in database
/// 
/// Serialization Format (append-only for forward compatibility):
///   [0..8]   balance (LE u64)
///   [8..16]  nonce (LE u64)
///   [16]     has_referrer flag (0|1)
///   [17..49] referrer addr (only if flag == 1)
///   [49..57] last_mined_height (LE u64)
///   [57..65] total_referred_miners (LE u64)
///   [65..73] total_referral_bonus_earned (LE u64)
///   [73..81] governance_weight (LE u64)
///   [81..89] total_blocks_mined (LE u64)
#[derive(Debug, Clone)]
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,
    pub referrer: Option<[u8; ADDRESS_BYTES]>,
    pub last_mined_height: u64,
    pub total_referred_miners: u64,
    pub total_referral_bonus_earned: u64,
    pub governance_weight: u64,
    pub total_blocks_mined: u64,
}

impl AccountState {
    pub fn empty() -> Self {
        AccountState {
            balance: 0,
            nonce: 0,
            referrer: None,
            last_mined_height: 0,
            total_referred_miners: 0,
            total_referral_bonus_earned: 0,
            governance_weight: 0,
            total_blocks_mined: 0,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(89);
        b.extend_from_slice(&self.balance.to_le_bytes());
        b.extend_from_slice(&self.nonce.to_le_bytes());

        match self.referrer {
            Some(r) => {
                b.push(1);
                b.extend_from_slice(&r);
            }
            None => {
                b.push(0);
            }
        }

        b.extend_from_slice(&self.last_mined_height.to_le_bytes());
        b.extend_from_slice(&self.total_referred_miners.to_le_bytes());
        b.extend_from_slice(&self.total_referral_bonus_earned.to_le_bytes());
        b.extend_from_slice(&self.governance_weight.to_le_bytes());
        b.extend_from_slice(&self.total_blocks_mined.to_le_bytes());
        b
    }

    pub fn from_bytes(d: &[u8]) -> Result<Self, &'static str> {
        if d.len() < 17 {
            return Err("account record too short");
        }

        let balance = u64::from_le_bytes(d[0..8].try_into().unwrap());
        let nonce = u64::from_le_bytes(d[8..16].try_into().unwrap());

        let mut off = 17usize;
        let referrer = if d[16] == 1 {
            if d.len() < 49 {
                return Err("truncated referrer");
            }
            let mut r = [0u8; 32];
            r.copy_from_slice(&d[17..49]);
            off = 49;
            Some(r)
        } else {
            None
        };

        let read_u64 = |o: usize, data: &[u8]| -> u64 {
            if data.len() >= o + 8 {
                u64::from_le_bytes(data[o..o + 8].try_into().unwrap())
            } else {
                0
            }
        };

        let last_mined_height = read_u64(off, d);
        off += 8;
        let total_referred_miners = read_u64(off, d);
        off += 8;
        let total_referral_bonus_earned = read_u64(off, d);
        off += 8;
        let governance_weight = read_u64(off, d);
        off += 8;
        let total_blocks_mined = read_u64(off, d);

        Ok(AccountState {
            balance,
            nonce,
            referrer,
            last_mined_height,
            total_referred_miners,
            total_referral_bonus_earned,
            governance_weight,
            total_blocks_mined,
        })
    }
}

impl Default for AccountState {
    fn default() -> Self {
        Self::empty()
    }
}

/// Block stored in database
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredBlock {
    pub version: [u8; 4],
    pub previous_hash: [u8; 32],
    pub merkle_root: [u8; 32],
    pub timestamp: [u8; 4],
    pub difficulty_target: [u8; 32],
    pub nonce: [u8; 8],
    pub block_height: [u8; 4],
    pub miner_address: [u8; 32],
    pub tx_data: Vec<StoredTransaction>,
}

impl StoredBlock {
    pub fn header_bytes(&self) -> [u8; 148] {
        let mut buf = [0u8; 148];
        buf[0..4].copy_from_slice(&self.version);
        buf[4..36].copy_from_slice(&self.previous_hash);
        buf[36..68].copy_from_slice(&self.merkle_root);
        buf[68..72].copy_from_slice(&self.timestamp);
        buf[72..104].copy_from_slice(&self.difficulty_target);
        buf[104..112].copy_from_slice(&self.nonce);
        buf[112..116].copy_from_slice(&self.block_height);
        buf[116..148].copy_from_slice(&self.miner_address);
        buf
    }

    pub fn header_prefix(&self) -> [u8; 140] {
        let mut buf = [0u8; 140];
        buf[0..4].copy_from_slice(&self.version);
        buf[4..36].copy_from_slice(&self.previous_hash);
        buf[36..68].copy_from_slice(&self.merkle_root);
        buf[68..72].copy_from_slice(&self.timestamp);
        buf[72..104].copy_from_slice(&self.difficulty_target);
        buf[104..108].copy_from_slice(&self.block_height);
        buf[108..140].copy_from_slice(&self.miner_address);
        buf
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&self.version);
        b.extend_from_slice(&self.previous_hash);
        b.extend_from_slice(&self.merkle_root);
        b.extend_from_slice(&self.timestamp);
        b.extend_from_slice(&self.difficulty_target);
        b.extend_from_slice(&self.nonce);
        b.extend_from_slice(&self.block_height);
        b.extend_from_slice(&self.miner_address);
        b.extend_from_slice(&(self.tx_data.len() as u32).to_le_bytes());
        for tx in &self.tx_data {
            b.extend_from_slice(&tx.to_bytes());
        }
        b
    }

    pub fn from_bytes(d: &[u8]) -> Result<Self, &'static str> {
        if d.len() < 148 {
            return Err("block header too short");
        }
        let mut off = 0usize;

        macro_rules! read {
            ($n:expr) => {{
                let slice = &d[off..off + $n];
                off += $n;
                slice
            }};
        }

        let mut version = [0u8; 4];
        version.copy_from_slice(read!(4));
        let mut previous_hash = [0u8; 32];
        previous_hash.copy_from_slice(read!(32));
        let mut merkle_root = [0u8; 32];
        merkle_root.copy_from_slice(read!(32));
        let mut timestamp = [0u8; 4];
        timestamp.copy_from_slice(read!(4));
        let mut difficulty_target = [0u8; 32];
        difficulty_target.copy_from_slice(read!(32));
        let mut nonce = [0u8; 8];
        nonce.copy_from_slice(read!(8));
        let mut block_height = [0u8; 4];
        block_height.copy_from_slice(read!(4));
        let mut miner_address = [0u8; 32];
        miner_address.copy_from_slice(read!(32));

        let mut tx_data = Vec::new();
        if d.len() >= off + 4 {
            let tx_count = u32::from_le_bytes(d[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            for _ in 0..tx_count {
                let (tx, n) = StoredTransaction::from_bytes(&d[off..])?;
                tx_data.push(tx);
                off += n;
            }
        }

        Ok(StoredBlock {
            version,
            previous_hash,
            merkle_root,
            timestamp: timestamp[0..4].try_into().unwrap(),
            difficulty_target,
            nonce,
            block_height: block_height[0..4].try_into().unwrap(),
            miner_address,
            tx_data,
        })
    }
}

/// Transaction stored in database
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredTransaction {
    pub version: u8,
    pub sender_address: [u8; 32],
    pub sender_pubkey: Vec<u8>,
    pub recipient_address: [u8; 32],
    pub amount: u64,
    pub fee: u64,
    pub nonce: u64,
    pub timestamp: u64,
    pub referrer_address: Option<[u8; 32]>,
    pub governance_data: Option<[u8; 32]>,
    pub signature: Vec<u8>,
}

impl StoredTransaction {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.push(self.version);
        b.extend_from_slice(&self.sender_address);
        b.extend_from_slice(&(self.sender_pubkey.len() as u32).to_le_bytes());
        b.extend_from_slice(&self.sender_pubkey);
        b.extend_from_slice(&self.recipient_address);
        b.extend_from_slice(&self.amount.to_le_bytes());
        b.extend_from_slice(&self.fee.to_le_bytes());
        b.extend_from_slice(&self.nonce.to_le_bytes());
        b.extend_from_slice(&self.timestamp.to_le_bytes());
        match self.referrer_address {
            Some(r) => {
                b.push(1);
                b.extend_from_slice(&r);
            }
            None => {
                b.push(0);
            }
        }
        match self.governance_data {
            Some(g) => {
                b.push(1);
                b.extend_from_slice(&g);
            }
            None => {
                b.push(0);
            }
        }
        b.extend_from_slice(&(self.signature.len() as u32).to_le_bytes());
        b.extend_from_slice(&self.signature);
        b
    }

    pub fn from_bytes(d: &[u8]) -> Result<(Self, usize), &'static str> {
        let mut off = 0usize;

        if d.is_empty() {
            return Err("tx too short: missing version");
        }
        let version = d[off];
        off += 1;

        if d.len() < off + 32 {
            return Err("tx: missing sender_address");
        }
        let mut sender_address = [0u8; 32];
        sender_address.copy_from_slice(&d[off..off + 32]);
        off += 32;

        if d.len() < off + 4 {
            return Err("tx: missing pubkey len");
        }
        let pk_len = u32::from_le_bytes(d[off..off + 4].try_into().unwrap()) as usize;
        off += 4;
        if d.len() < off + pk_len {
            return Err("tx: missing pubkey data");
        }
        let sender_pubkey = d[off..off + pk_len].to_vec();
        off += pk_len;

        if d.len() < off + 32 {
            return Err("tx: missing recipient");
        }
        let mut recipient_address = [0u8; 32];
        recipient_address.copy_from_slice(&d[off..off + 32]);
        off += 32;

        if d.len() < off + 32 {
            return Err("tx: missing core scalar fields");
        }
        let amount = u64::from_le_bytes(d[off..off + 8].try_into().unwrap());
        off += 8;
        let fee = u64::from_le_bytes(d[off..off + 8].try_into().unwrap());
        off += 8;
        let nonce = u64::from_le_bytes(d[off..off + 8].try_into().unwrap());
        off += 8;
        let timestamp = u64::from_le_bytes(d[off..off + 8].try_into().unwrap());
        off += 8;

        let referrer_address = if d.len() > off {
            let flag = d[off];
            off += 1;
            if flag == 1 {
                if d.len() < off + 32 {
                    return Err("tx: truncated referrer address");
                }
                let mut r = [0u8; 32];
                r.copy_from_slice(&d[off..off + 32]);
                off += 32;
                Some(r)
            } else {
                None
            }
        } else {
            None
        };

        let governance_data = if d.len() > off {
            let flag = d[off];
            off += 1;
            if flag == 1 {
                if d.len() < off + 32 {
                    return Err("tx: truncated governance data");
                }
                let mut g = [0u8; 32];
                g.copy_from_slice(&d[off..off + 32]);
                off += 32;
                Some(g)
            } else {
                None
            }
        } else {
            None
        };

        let signature = if d.len() >= off + 4 {
            let sig_len = u32::from_le_bytes(d[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            if d.len() < off + sig_len {
                return Err("tx: truncated signature");
            }
            let s = d[off..off + sig_len].to_vec();
            off += sig_len;
            s
        } else {
            vec![]
        };

        Ok((
            StoredTransaction {
                version,
                sender_address,
                sender_pubkey,
                recipient_address,
                amount,
                fee,
                nonce,
                timestamp,
                referrer_address,
                governance_data,
                signature,
            },
            off,
        ))
    }
}
