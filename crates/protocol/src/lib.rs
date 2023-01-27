mod message;
mod object;
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use bytes::{Buf, BufMut, BytesMut};
use flate2::Compression;
use flate2::{read::ZlibDecoder, write::ZlibEncoder};
pub use message::*;
pub use object::*;
use rand::Rng;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

const MAGIC: [u8; 4] = [0x23, 0x33, 0x23, 0x33];

// 4 bytes magic + 4 bytes data length + 12 bytes nonce
pub const HEADER_LEN: usize = 4 + 4 + 12;

pub type Key = [u8; 32];

pub fn make_key(key: &str) -> Key {
    let hash = Sha256::digest(key);
    hash.into()
}

fn random_nonce() -> [u8; 12] {
    let mut nonce = [0u8; 12];
    let mut rng = rand::thread_rng();
    rng.fill(&mut nonce);
    nonce
}

pub fn encode(t: impl Serialize, buf: &mut BytesMut, key: &Key) -> bool {
    let bytes = match bincode::serialize(&t) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("serialize error: {}", e);
            return false;
        }
    };

    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    if let Err(e) = e.write_all(&bytes) {
        log::error!("compress error: {}", e);
        return false;
    }
    let bytes = match e.finish() {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("compress error: {}", e);
            return false;
        }
    };

    let cipher = Aes256Gcm::new(key.into());
    let nonce = random_nonce();
    let encbytes = match cipher.encrypt(&Nonce::from(nonce), bytes.as_ref()) {
        Ok(encbytes) => encbytes,
        Err(e) => {
            log::error!("encrypt error: {}", e);
            return false;
        }
    };

    buf.reserve(4 + 4 + encbytes.len());
    buf.put_slice(&MAGIC);
    buf.put_u32(encbytes.len() as u32);
    buf.put_slice(&nonce);
    buf.put_slice(&encbytes);

    true
}

pub enum DecodeError {
    NotEnoughData,
    InvalidData,
}

pub fn decode<T>(buf: &mut BytesMut, key: &Key) -> Result<T, DecodeError>
where
    T: DeserializeOwned,
{
    let magic = &buf[..4];
    if magic != MAGIC {
        return Err(DecodeError::InvalidData);
    }

    let len = &buf[4..8];
    let len = u32::from_be_bytes([len[0], len[1], len[2], len[3]]) as usize;
    if buf.len() < HEADER_LEN + len {
        return Err(DecodeError::NotEnoughData);
    }

    let nonce = &buf[8..20];
    let nonce = Nonce::from_slice(nonce).to_owned();

    buf.advance(HEADER_LEN);
    let encbytes = buf.split_to(len);

    // decrypt use aes256gcm
    let cipher = Aes256Gcm::new(key.into());
    let bytes = match cipher.decrypt(&nonce, encbytes.as_ref()) {
        Ok(bytes) => bytes,
        Err(_) => return Err(DecodeError::InvalidData),
    };

    let mut zd = ZlibDecoder::new(&bytes[..]);
    let mut objbytes = Vec::new();
    zd.read_to_end(&mut objbytes).unwrap();

    let msg = match bincode::deserialize(&objbytes) {
        Ok(msg) => msg,
        Err(_) => return Err(DecodeError::InvalidData),
    };

    Ok(msg)
}
