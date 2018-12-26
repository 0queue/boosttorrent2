use byteorder::{ByteOrder, NetworkEndian};
use bytes::{Buf, BufMut, Bytes, BytesMut, IntoBuf};
use std::io;
use tokio::codec::{Decoder, Encoder};
use actix::Message;

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

#[derive(Message)]
pub enum BitTorrentMessage {
    Handshake(Handshake),
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

impl Decoder for MessageCodec {
    type Item = BitTorrentMessage;
    type Error = io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {

        if src.len() >= (1 + 19 + 8 + 20 + 20) && &src[0..20] == b"\x13BitTorrent protocol" {
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

            Ok(Some(BitTorrentMessage::Handshake((info_hash, peer_id).into())))
        } else {
            let length = NetworkEndian::read_u32(&src.split_to(4)) as usize;
            if src.len() < length {
                return Ok(None);
            }
            let mut buf = src.split_to(length).into_buf();
            let type_id = buf.get_u8();

            let message = match type_id {
                0 => Some(BitTorrentMessage::Choke),
                1 => Some(BitTorrentMessage::Unchoke),
                2 => Some(BitTorrentMessage::Interested),
                3 => Some(BitTorrentMessage::NotInterested),
                4 => Some(BitTorrentMessage::Have(buf.get_u32_be())),
                5 => {
                    let mut bytes = Vec::with_capacity(length - 1);
                    buf.copy_to_slice(&mut bytes);
                    Some(BitTorrentMessage::Bitfield(bit_vec::BitVec::from_bytes(&bytes)))
                }
                6 => {
                    let index = buf.get_u32_be();
                    let begin = buf.get_u32_be();
                    let length = buf.get_u32_be();
                    Some(BitTorrentMessage::Request((index, begin, length).into()))
                }
                7 => {
                    let index = buf.get_u32_be();
                    let begin = buf.get_u32_be();
                    let block = buf.collect();
                    Some(BitTorrentMessage::Piece(Piece::new(index, begin, block)))
                }
                8 => {
                    let index = buf.get_u32_be();
                    let begin = buf.get_u32_be();
                    let length = buf.get_u32_be();
                    Some(BitTorrentMessage::Cancel((index, begin, length).into()))
                }
                _ => None,
            };

            message.ok_or(io::Error::new(io::ErrorKind::Other, "Invalid message id"))
                .map(|m| Some(m))
        }
    }
}

impl Encoder for MessageCodec {
    type Item = BitTorrentMessage;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            BitTorrentMessage::Handshake(item) => {
                dst.reserve(1 + 19 + 8 + 20 + 20);

                dst.put(19u8);
                dst.put(b"BitTorrent protocol".as_ref());
                dst.put([0u8; 8].as_ref());
                dst.put(item.info_hash.as_ref());
                dst.put(item.peer_id.as_ref());
            },
            BitTorrentMessage::Choke => length_and_id(dst, 1, 0),
            BitTorrentMessage::Unchoke => length_and_id(dst, 1, 1),
            BitTorrentMessage::Interested => length_and_id(dst, 1, 2),
            BitTorrentMessage::NotInterested => length_and_id(dst, 1, 3),
            BitTorrentMessage::Have(piece_index) => {
                length_and_id(dst, 5, 4);
                dst.put_u32_be(piece_index);
            }
            BitTorrentMessage::Bitfield(bit_vec) => {
                length_and_id(dst, 1 + bit_vec.len() as u32, 5);
                dst.put(&bit_vec.to_bytes());
            }
            BitTorrentMessage::Request(request) => {
                length_and_id(dst, 13, 6);
                dst.put_u32_be(request.index);
                dst.put_u32_be(request.begin);
                dst.put_u32_be(request.length);
            }
            BitTorrentMessage::Piece(piece) => {
                length_and_id(dst, 9 + piece.block.len() as u32, 7);
                dst.put_u32_be(piece.index);
                dst.put_u32_be(piece.begin);
                dst.put(&piece.block);
            }
            BitTorrentMessage::Cancel(request) => {
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