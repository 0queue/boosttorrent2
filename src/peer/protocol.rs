use std::io;

use byteorder::{ByteOrder, NetworkEndian};
use bytes::{BufMut, Bytes, BytesMut};
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

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let length = NetworkEndian::read_u32(&src.split_to(4)) as usize;
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

        if !src.is_empty() {
            return Err(io::Error::new(io::ErrorKind::Other, "Extra bytes"));
        }

        message.ok_or(io::Error::new(io::ErrorKind::Other, "Invalid message"))
            .map(|m| Some(m))
    }
}

impl Encoder for MessageCodec {
    type Item = Message;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // Note: maybe should reserve capacity?
        match item {
            Message::Choke => length_and_id(dst, 1, 0),
            Message::Unchoke => length_and_id(dst, 1, 1),
            Message::Interested => length_and_id(dst, 1, 2),
            Message::NotInterested => length_and_id(dst, 1, 3),
            Message::Have(piece_index) => {
                length_and_id(dst, 5, 4);
                NetworkEndian::write_u32(dst, piece_index);
            }
            Message::Bitfield(bit_vec) => {
                length_and_id(dst, 1 + bit_vec.len() as u32, 5);
                dst.extend_from_slice(&bit_vec.to_bytes());
            }
            Message::Request(request) => {
                length_and_id(dst, 13, 6);
                NetworkEndian::write_u32(dst, request.index);
                NetworkEndian::write_u32(dst, request.begin);
                NetworkEndian::write_u32(dst, request.length);
            }
            Message::Piece(piece) => {
                length_and_id(dst, 9 + piece.block.len() as u32, 7);
                NetworkEndian::write_u32(dst, piece.index);
                NetworkEndian::write_u32(dst, piece.begin);
                dst.extend_from_slice(&piece.block);
            }
            Message::Cancel(request) => {
                length_and_id(dst, 13, 8);
                NetworkEndian::write_u32(dst, request.index);
                NetworkEndian::write_u32(dst, request.begin);
                NetworkEndian::write_u32(dst, request.length);
            }
        }

        Ok(())
    }
}

fn length_and_id(dst: &mut BytesMut, length: u32, id: u8) {
    NetworkEndian::write_u32(dst, length);
    dst.put_u8(id);
}