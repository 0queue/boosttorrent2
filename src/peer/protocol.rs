use std::io;

use byteorder::{ByteOrder, NetworkEndian};
use bytes::{BufMut, Bytes, BytesMut};
use tokio::codec::{Decoder, Encoder};

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
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

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            return Ok(None);
        }

        let length = NetworkEndian::read_u32(&src[..4]) as usize;

        if src.len() < length {
            return Ok(None);
        }

        // advance after len check so that the len check can happen again
        // after more bytes are available
        src.advance(4);
        let type_id = src.split_to(1)[0];

        let message = match type_id {
            0 => Some(Message::Choke),
            1 => Some(Message::Unchoke),
            2 => Some(Message::Interested),
            3 => Some(Message::NotInterested),
            4 => Some(Message::Have(NetworkEndian::read_u32(&src.split_to(4)))),
            5 => Some(Message::Bitfield(bit_vec::BitVec::from_bytes(&src.split_to(length - 1)))),
            6 => {
                let index = NetworkEndian::read_u32(&src.split_to(4));
                let begin = NetworkEndian::read_u32(&src.split_to(4));
                let length = NetworkEndian::read_u32(&src.split_to(4));
                Some(Message::Request((index, begin, length).into()))
            }
            7 => {
                let index = NetworkEndian::read_u32(&src.split_to(4));
                let begin = NetworkEndian::read_u32(&src.split_to(4));
                let block = src.split_to(length - 9);
                Some(Message::Piece(Piece::new(index, begin, block.freeze())))
            }
            8 => {
                let index = NetworkEndian::read_u32(&src.split_to(4));
                let begin = NetworkEndian::read_u32(&src.split_to(4));
                let length = NetworkEndian::read_u32(&src.split_to(4));
                Some(Message::Cancel((index, begin, length).into()))
            }
            _ => None,
        };

        message.ok_or(io::Error::new(io::ErrorKind::Other, "Invalid message"))
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
                length_and_id(dst, 4, 4);
                dst.put_slice(&network_endian(piece_index))
            }
            Message::Bitfield(bit_vec) => {
                length_and_id(dst, bit_vec.len() as u32, 5);
                dst.put_slice(&bit_vec.to_bytes());
            }
            Message::Request(request) => {
                length_and_id(dst, 12, 6);
                dst.put_slice(&network_endian(request.index));
                dst.put_slice(&network_endian(request.begin));
                dst.put_slice(&network_endian(request.length));
            }
            Message::Piece(piece) => {
                length_and_id(dst, 8 + piece.block.len() as u32, 7);
                dst.put_slice(&network_endian(piece.index));
                dst.put_slice(&network_endian(piece.begin));
                dst.put_slice(&piece.block);
            }
            Message::Cancel(request) => {
                length_and_id(dst, 12, 8);
                dst.put_slice(&network_endian(request.index));
                dst.put_slice(&network_endian(request.begin));
                dst.put_slice(&network_endian(request.length));
            }
        }

        Ok(())
    }
}

/// write out the length of the message, reserve as many bytes as needed
fn length_and_id(dst: &mut BytesMut, length: u32, id: u8) {
    dst.reserve(4 + 1 + length as usize);
    dst.put_slice(&network_endian(length));
    dst.put_u8(id);
}

fn network_endian(n: u32) -> [u8; 4] {
    let mut buf = [0; 4];
    NetworkEndian::write_u32(&mut buf, n);
    buf
}