use std::io;

use bytes::{BufMut, BytesMut};
use tokio::codec::{Decoder, Encoder};

pub struct Handshake {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl From<([u8; 20], [u8; 20])> for Handshake {
    fn from(pair: ([u8; 20], [u8; 20])) -> Self {
        Handshake {
            info_hash: pair.0,
            peer_id: pair.1,
        }
    }
}

pub struct HandshakeCodec;

impl HandshakeCodec {
    pub fn new() -> HandshakeCodec {
        HandshakeCodec {}
    }
}

/// Bittorrent handshake structure:
/// length byte (19)
/// 19 bytes ('BitTorrent protocol')
/// 8 empty bytes
/// 20 bytes info_hash
/// 20 bytes peer_id
impl Decoder for HandshakeCodec {
    type Item = Handshake;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let length = src.split_to(1);
        if length != &vec![19] {
            return Err(io::Error::new(io::ErrorKind::Other, "invalid handshake name length"));
        }

        let name = src.split_to(19);
        if name != b"BitTorrent protocol".as_ref() {
            return Err(io::Error::new(io::ErrorKind::Other, "invalid protocol name"));
        }

        let empties = src.split_to(8);
        if empties != [0u8; 8].as_ref() {
            return Err(io::Error::new(io::ErrorKind::Other, "invalid protocol extension"));
        }

        let info_hash = {
            let mut info_hash = [0u8; 20];
            info_hash.copy_from_slice(src.split_to(20).as_ref());
            info_hash
        };

        let peer_id = {
            let mut peer_id = [0u8; 20];
            peer_id.copy_from_slice(src.split_to(20).as_ref());
            peer_id
        };

        if !src.is_empty() {
            return Err(io::Error::new(io::ErrorKind::Other, "invalid handshake, too much data"));
        }

        Ok(Some((info_hash, peer_id).into()))
    }
}

impl Encoder for HandshakeCodec {
    type Item = Handshake;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(1 + 19 + 8 + 20 + 20);

        dst.put(19u8);
        dst.put(b"BitTorrent protocol".as_ref());
        dst.put([0u8; 8].as_ref());
        dst.put(item.info_hash.as_ref());
        dst.put(item.peer_id.as_ref());

        Ok(())
    }
}