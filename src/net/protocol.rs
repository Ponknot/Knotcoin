// Wire protocol — all messages are length-prefixed frames:
//   [4 bytes LE message length][message bytes]
//
// StoredBlock is transmitted as its canonical byte representation
// (via StoredBlock::to_bytes / from_bytes), NOT via bincode. This ensures
// the bytes on the wire are identical to what is stored in the chain DB,
// so received blocks can be stored directly without re-serialization.

use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::node::db_common::StoredBlock;

const MAGIC: [u8; 4] = [0x4B, 0x4E, 0x4F, 0x54]; // "KNOT"
// SECURITY FIX: Reduced from 8MB to 1MB to prevent memory exhaustion DoS
// Max block size is 500KB, so 1MB provides sufficient overhead while preventing
// malicious peers from forcing nodes to allocate excessive memory buffers
const MAX_FRAME: usize = 1 * 1024 * 1024; // 1 MB safety limit

#[derive(Debug, Clone)]
pub enum NetworkMessage {
    Version { height: u32 },
    Verack,
    GetHeaders { from_hash: [u8; 32] },
    Headers(Vec<[u8; 32]>),
    GetBlocks { hashes: Vec<[u8; 32]> },
    Blocks(Vec<Vec<u8>>), // each inner Vec is raw StoredBlock bytes
    Ping(u64),
    Pong(u64),
    Challenge([u8; 32]),
    Response([u8; 32]),
    Addr(Vec<SocketAddr>),
    GetAddr, // Request peers from connected node
    Tx(Vec<u8>), // raw transaction bytes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum MsgType {
    Version = 0x01,
    Verack = 0x02,
    GetHeaders = 0x10,
    Headers = 0x11,
    GetBlocks = 0x12,
    Blocks = 0x13,
    Ping = 0x20,
    Pong = 0x21,
    Challenge = 0x30,
    Response = 0x31,
    Addr = 0x40,
    GetAddr = 0x41,
    Tx = 0x50,
}

impl MsgType {
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Version),
            0x02 => Some(Self::Verack),
            0x10 => Some(Self::GetHeaders),
            0x11 => Some(Self::Headers),
            0x12 => Some(Self::GetBlocks),
            0x13 => Some(Self::Blocks),
            0x20 => Some(Self::Ping),
            0x21 => Some(Self::Pong),
            0x30 => Some(Self::Challenge),
            0x31 => Some(Self::Response),
            0x40 => Some(Self::Addr),
            0x41 => Some(Self::GetAddr),
            0x50 => Some(Self::Tx),
            _ => None,
        }
    }
}

fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

fn write_hashes(buf: &mut Vec<u8>, hashes: &[[u8; 32]]) {
    write_u32(buf, hashes.len() as u32);
    for h in hashes {
        buf.extend_from_slice(h);
    }
}

fn read_u32(d: &[u8], off: &mut usize) -> Option<u32> {
    if d.len() < *off + 4 {
        return None;
    }
    let v = u32::from_le_bytes(d[*off..*off + 4].try_into().unwrap());
    *off += 4;
    Some(v)
}

fn read_u64(d: &[u8], off: &mut usize) -> Option<u64> {
    if d.len() < *off + 8 {
        return None;
    }
    let v = u64::from_le_bytes(d[*off..*off + 8].try_into().unwrap());
    *off += 8;
    Some(v)
}

fn read_hash(d: &[u8], off: &mut usize) -> Option<[u8; 32]> {
    if d.len() < *off + 32 {
        return None;
    }
    let mut h = [0u8; 32];
    h.copy_from_slice(&d[*off..*off + 32]);
    *off += 32;
    Some(h)
}

fn read_hashes(d: &[u8], off: &mut usize) -> Option<Vec<[u8; 32]>> {
    let count = read_u32(d, off)? as usize;
    if count > 2000 {
        return None;
    }
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        out.push(read_hash(d, off)?);
    }
    Some(out)
}

impl NetworkMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        match self {
            NetworkMessage::Version { height } => {
                payload.push(MsgType::Version as u8);
                write_u32(&mut payload, *height);
            }
            NetworkMessage::Verack => {
                payload.push(MsgType::Verack as u8);
            }
            NetworkMessage::GetHeaders { from_hash } => {
                payload.push(MsgType::GetHeaders as u8);
                payload.extend_from_slice(from_hash);
            }
            NetworkMessage::Headers(hashes) => {
                payload.push(MsgType::Headers as u8);
                write_hashes(&mut payload, hashes);
            }
            NetworkMessage::GetBlocks { hashes } => {
                payload.push(MsgType::GetBlocks as u8);
                write_hashes(&mut payload, hashes);
            }
            NetworkMessage::Blocks(blocks) => {
                // Each block is stored as [4-byte LE length][raw bytes].
                // This is byte-for-byte identical to ChainDB storage.
                payload.push(MsgType::Blocks as u8);
                write_u32(&mut payload, blocks.len() as u32);
                for raw in blocks {
                    write_u32(&mut payload, raw.len() as u32);
                    payload.extend_from_slice(raw);
                }
            }
            NetworkMessage::Ping(n) => {
                payload.push(MsgType::Ping as u8);
                write_u64(&mut payload, *n);
            }
            NetworkMessage::Pong(n) => {
                payload.push(MsgType::Pong as u8);
                write_u64(&mut payload, *n);
            }
            NetworkMessage::Response(r) => {
                payload.push(MsgType::Response as u8);
                payload.extend_from_slice(r);
            }
            NetworkMessage::Challenge(c) => {
                payload.push(MsgType::Challenge as u8);
                payload.extend_from_slice(c);
            }
            NetworkMessage::Addr(addrs) => {
                payload.push(MsgType::Addr as u8);
                write_u32(&mut payload, addrs.len() as u32);
                for addr in addrs {
                    match addr {
                        std::net::SocketAddr::V4(v4) => {
                            payload.push(0x04);
                            payload.extend_from_slice(&v4.ip().octets());
                            payload.extend_from_slice(&v4.port().to_be_bytes());
                        }
                        std::net::SocketAddr::V6(v6) => {
                            payload.push(0x06);
                            payload.extend_from_slice(&v6.ip().octets());
                            payload.extend_from_slice(&v6.port().to_be_bytes());
                        }
                    }
                }
            }
            NetworkMessage::GetAddr => {
                payload.push(MsgType::GetAddr as u8);
            }
            NetworkMessage::Tx(raw) => {
                payload.push(MsgType::Tx as u8);
                payload.extend_from_slice(raw);
            }
        }

        // Frame: MAGIC[4] + length[4] + payload
        let mut frame = Vec::with_capacity(8 + payload.len());
        frame.extend_from_slice(&MAGIC);
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(&payload);
        frame
    }

    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 9 {
            return None;
        }
        if data[..4] != MAGIC {
            return None;
        }
        let payload_len = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        if data.len() < 8 + payload_len {
            return None;
        }
        let payload = &data[8..8 + payload_len];

        let type_byte = payload[0];
        let body = &payload[1..];
        let mut off = 0usize;

        match MsgType::from_u8(type_byte)? {
            MsgType::Version => {
                let height = read_u32(body, &mut off)?;
                Some(NetworkMessage::Version { height })
            }
            MsgType::Verack => Some(NetworkMessage::Verack),
            MsgType::GetHeaders => {
                let mut off2 = 0;
                let from_hash = read_hash(body, &mut off2)?;
                Some(NetworkMessage::GetHeaders { from_hash })
            }
            MsgType::Headers => {
                let hashes = read_hashes(body, &mut off)?;
                Some(NetworkMessage::Headers(hashes))
            }
            MsgType::GetBlocks => {
                let hashes = read_hashes(body, &mut off)?;
                Some(NetworkMessage::GetBlocks { hashes })
            }
            MsgType::Blocks => {
                let count = read_u32(body, &mut off)? as usize;
                if count > 500 {
                    return None;
                }
                let mut blocks = Vec::with_capacity(count);
                for _ in 0..count {
                    let len = read_u32(body, &mut off)? as usize;
                    if body.len() < off + len {
                        return None;
                    }
                    blocks.push(body[off..off + len].to_vec());
                    off += len;
                }
                Some(NetworkMessage::Blocks(blocks))
            }
            MsgType::Ping => Some(NetworkMessage::Ping(read_u64(body, &mut off)?)),
            MsgType::Pong => Some(NetworkMessage::Pong(read_u64(body, &mut off)?)),
            MsgType::Response => {
                if body.len() < 32 {
                    return None;
                }
                let mut r = [0u8; 32];
                r.copy_from_slice(&body[0..32]);
                Some(NetworkMessage::Response(r))
            }
            MsgType::Challenge => {
                if body.len() < 32 {
                    return None;
                }
                let mut c = [0u8; 32];
                c.copy_from_slice(&body[0..32]);
                Some(NetworkMessage::Challenge(c))
            }
            MsgType::Addr => {
                let count = read_u32(body, &mut off)? as usize;
                if count > 1000 { return None; }
                let mut addrs = Vec::with_capacity(count);
                for _ in 0..count {
                    if off >= body.len() {
                        return None;
                    }
                    let ty = body[off];
                    off += 1;
                    if ty == 0x04 {
                        if body.len() < off + 4 + 2 {
                            return None;
                        }
                        let mut ip = [0u8; 4];
                        ip.copy_from_slice(&body[off..off+4]);
                        off += 4;
                        let port = u16::from_be_bytes(body[off..off+2].try_into().unwrap());
                        off += 2;
                        addrs.push(std::net::SocketAddr::new(std::net::IpAddr::V4(ip.into()), port));
                    } else if ty == 0x06 {
                        if body.len() < off + 16 + 2 {
                            return None;
                        }
                        let mut ip = [0u8; 16];
                        ip.copy_from_slice(&body[off..off+16]);
                        off += 16;
                        let port = u16::from_be_bytes(body[off..off+2].try_into().unwrap());
                        off += 2;
                        addrs.push(std::net::SocketAddr::new(std::net::IpAddr::V6(ip.into()), port));
                    } else { return None; }
                }
                Some(NetworkMessage::Addr(addrs))
            }
            MsgType::GetAddr => {
                Some(NetworkMessage::GetAddr)
            }
            MsgType::Tx => {
                Some(NetworkMessage::Tx(body.to_vec()))
            }
        }
    }

    // Convenience: unwrap a Blocks message into parsed StoredBlock list
    pub fn into_stored_blocks(self) -> Option<Vec<StoredBlock>> {
        if let NetworkMessage::Blocks(raws) = self {
            raws.iter()
                .map(|raw| StoredBlock::from_bytes(raw).ok())
                .collect()
        } else {
            None
        }
    }
}

pub struct FramedStream {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl FramedStream {
    pub fn new(stream: TcpStream) -> Self {
        FramedStream {
            stream,
            buf: Vec::new(),
        }
    }

    pub async fn send(&mut self, msg: &NetworkMessage) -> io::Result<()> {
        self.stream.write_all(&msg.encode()).await
    }

    pub async fn recv(&mut self) -> io::Result<Option<NetworkMessage>> {
        loop {
            // Do we have a full frame already buffered?
            if self.buf.len() >= 8 {
                let payload_len = u32::from_le_bytes(self.buf[4..8].try_into().unwrap()) as usize;
                let frame_len = 8 + payload_len;

                if payload_len > MAX_FRAME {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "frame too large",
                    ));
                }

                if self.buf.len() >= frame_len {
                    let frame = self.buf[..frame_len].to_vec();
                    self.buf.drain(..frame_len);
                    return Ok(NetworkMessage::decode(&frame));
                }
            }

            // Need more data
            let mut tmp = vec![0u8; 4096];
            let n = self.stream.read(&mut tmp).await?;
            if n == 0 {
                return Ok(None);
            }
            self.buf.extend_from_slice(&tmp[..n]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(msg: NetworkMessage) -> NetworkMessage {
        let enc = msg.encode();
        NetworkMessage::decode(&enc).expect("decode failed")
    }

    #[test]
    fn test_version() {
        let m = roundtrip(NetworkMessage::Version { height: 12345 });
        if let NetworkMessage::Version { height } = m {
            assert_eq!(height, 12345);
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_blocks_roundtrip() {
        let raw1 = vec![0xABu8; 148];
        let raw2 = vec![0xCDu8; 160];
        let m = roundtrip(NetworkMessage::Blocks(vec![raw1.clone(), raw2.clone()]));
        if let NetworkMessage::Blocks(blocks) = m {
            assert_eq!(blocks[0], raw1);
            assert_eq!(blocks[1], raw2);
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_get_headers() {
        let h = [0x42u8; 32];
        let m = roundtrip(NetworkMessage::GetHeaders { from_hash: h });
        if let NetworkMessage::GetHeaders { from_hash } = m {
            assert_eq!(from_hash, h);
        } else {
            panic!("wrong type");
        }
    }

    #[test]
    fn test_ping_pong() {
        let m = roundtrip(NetworkMessage::Ping(9999999));
        if let NetworkMessage::Ping(n) = m {
            assert_eq!(n, 9999999);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_bad_magic_rejected() {
        let mut enc = NetworkMessage::Verack.encode();
        enc[0] = 0xFF;
        assert!(NetworkMessage::decode(&enc).is_none());
    }
}
