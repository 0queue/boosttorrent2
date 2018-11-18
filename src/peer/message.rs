use byteorder::{ByteOrder, NetworkEndian};
use bytes::{Buf, BufMut, Bytes, BytesMut, IntoBuf};
use derive_error::Error;
use std::io;
use tokio::codec::{Decoder, Encoder};

pub struct Request {
    index: u32,
    begin: u32,
    length: u32,
}

impl From<(u32, u32, u32)> for Request {
    fn from((index, begin, length): (u32, u32, u32)) -> Self {
        Request { index, begin, length }
    }
}

pub struct Piece {
    index: u32,
    begin: u32,
    block: Bytes,
}

impl Piece {
    pub fn new(index: u32, begin: u32, block: Bytes) -> Piece {
        Piece { index, begin, block }
    }
}

pub enum Message {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(bit_vec::BitVec),
    Request(Request),
    Piece(Piece),
    Cancel(Request),
}

pub struct MessageCodec;


impl MessageCodec {
    pub fn new() -> MessageCodec {
        MessageCodec {}
    }
}

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

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // return Ok(None) if a frame is not fully available yet
        if src.len() < 4 {
            return Ok(None);
        }
        let length = NetworkEndian::read_u32(&src.split_to(4)) as usize;
        if src.len() < length {
            return Ok(None);
        }
        let mut buf = src.split_to(length).into_buf();
        let type_id = buf.get_u8();

        let message = match type_id {
            0 => Some(Message::Choke),
            1 => Some(Message::Unchoke),
            2 => Some(Message::Interested),
            3 => Some(Message::NotInterested),
            4 => Some(Message::Have(buf.get_u32_be())),
            5 => {
                let mut bytes = Vec::with_capacity(length - 1);
                buf.copy_to_slice(&mut bytes);
                Some(Message::Bitfield(bit_vec::BitVec::from_bytes(&bytes)))
            }
            6 => {
                let index = buf.get_u32_be();
                let begin = buf.get_u32_be();
                let length = buf.get_u32_be();
                Some(Message::Request((index, begin, length).into()))
            }
            7 => {
                let index = buf.get_u32_be();
                let begin = buf.get_u32_be();
                let block = buf.collect();
                Some(Message::Piece(Piece::new(index, begin, block)))
            }
            8 => {
                let index = buf.get_u32_be();
                let begin = buf.get_u32_be();
                let length = buf.get_u32_be();
                Some(Message::Cancel((index, begin, length).into()))
            }
            _ => None,
        };

        message.ok_or(io::Error::new(io::ErrorKind::Other, "Invalid message id"))
            .map(|m| Some(m))
    }
}

impl Encoder for MessageCodec {
    type Item = Message;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            Message::Choke => length_and_id(dst, 1, 0),
            Message::Unchoke => length_and_id(dst, 1, 1),
            Message::Interested => length_and_id(dst, 1, 2),
            Message::NotInterested => length_and_id(dst, 1, 3),
            Message::Have(piece_index) => {
                length_and_id(dst, 5, 4);
                dst.put_u32_be(piece_index);
            }
            Message::Bitfield(bit_vec) => {
                length_and_id(dst, 1 + bit_vec.len() as u32, 5);
                dst.put(&bit_vec.to_bytes());
            }
            Message::Request(request) => {
                length_and_id(dst, 13, 6);
                dst.put_u32_be(request.index);
                dst.put_u32_be(request.begin);
                dst.put_u32_be(request.length);
            }
            Message::Piece(piece) => {
                length_and_id(dst, 9 + piece.block.len() as u32, 7);
                dst.put_u32_be(piece.index);
                dst.put_u32_be(piece.begin);
                dst.put(&piece.block);
            }
            Message::Cancel(request) => {
                length_and_id(dst, 13, 8);
                dst.put_u32_be(request.index);
                dst.put_u32_be(request.begin);
                dst.put_u32_be(request.length);
            }
        }
        Ok(())
    }
}

fn length_and_id(dst: &mut BytesMut, length: u32, id: u8) {
    dst.reserve((length + 4) as usize);
    dst.put_u32_be(length);
    dst.put_u8(id);
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
        // The entire handshake isn't in yet, so return None
        if src.len() < (1 + 19 + 8 + 20 + 20) {
            return Ok(None);
        }
        let src = src.split_to(1 + 19 + 8 + 20 + 20);
        let mut buf = src.into_buf();
        if buf.get_u8() != 19 {
            return Err(io::Error::new(io::ErrorKind::Other, "invalid handshake name length"));
        }

        let mut name: [u8; 19] = [0; 19];
        buf.copy_to_slice(&mut name);
        if name != b"BitTorrent protocol".as_ref() {
            return Err(io::Error::new(io::ErrorKind::Other, "invalid protocol name"));
        }

        // read reserved bytes
        buf.copy_to_slice(&mut name[0..8]);

        let mut info_hash: [u8; 20] = [0; 20];
        buf.copy_to_slice(&mut info_hash);

        let mut peer_id: [u8; 20] = [0; 20];
        buf.copy_to_slice(&mut peer_id);

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