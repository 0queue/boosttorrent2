use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;

use byteorder::{ByteOrder, NetworkEndian};
use futures::{
    future::Future,
    sync::mpsc::UnboundedReceiver,
    sync::mpsc::UnboundedSender,
};
use futures::future::IntoFuture;
use futures::stream::Stream;
use futures::sync::mpsc;
use tokio::codec::Framed;
use tokio::net::tcp::ConnectFuture;
use tokio::net::TcpStream;

use boostencode::{
    FromValue,
    Value,
};

#[derive(Debug)]
pub struct PeerInfo {
    addr: SocketAddr,
    peer_id: Option<[u8; 20]>,
}

pub struct Peer {
    info: PeerInfo,
    pub output: UnboundedSender<u8>,
    pub input: UnboundedReceiver<u8>,
}

impl FromValue for PeerInfo {
    type Error = String;

    fn from_value(val: &Value) -> Result<Self, Self::Error> where Self: Sized {
        let map = val.dict().ok_or("Not a dictionary".to_string())?;

        let mut peer_id = [0u8; 20];
        map.get("peer id".as_bytes()).and_then(Value::bstring)
            .map(|bytes| peer_id.copy_from_slice(bytes))
            .ok_or("Missing key: peer id".to_string())?;
        let peer_id = Some(peer_id);

        let ip = map.get("ip".as_bytes()).and_then(Value::bstring_utf8)
            .map(|s| s.parse())
            .ok_or("Missing key: ip".to_string())?
            .map_err(|_| "Invalid ip addr".to_string())?;

        let port = map.get("port".as_bytes()).and_then(Value::integer)
            .map(|i| *i as u16)
            .ok_or("Missing key: port".to_string())?;

        Ok(PeerInfo { addr: SocketAddr::new(ip, port), peer_id })
    }
}

impl PeerInfo {
    pub fn from_compact(bytes: &[u8]) -> Vec<PeerInfo> {
        bytes.chunks(6).map(|chunk| {
            let port = NetworkEndian::read_u16(&chunk[4..]);
            let mut ip = [0; 4];
            ip.copy_from_slice(&chunk[..4]);

            PeerInfo {
                addr: (ip, port).into(),
                peer_id: None,
            }
        }).collect()
    }

    pub fn connect(&self) -> ConnectFuture {
        TcpStream::connect(&self.addr)
    }
}

impl Peer {
    pub fn handshake(socket: &mut TcpStream, info_hash: [u8; 20]) -> (UnboundedReceiver<u8>, UnboundedSender<u8>) {
        let (tx, rx) = mpsc::unbounded();

        let (write, read) = Framed::new(socket, handshake::HandshakeCodec::new()).split();


        return (rx, tx);
    }
}

mod handshake {
    use std::io;

    use bytes::BufMut;
    use bytes::BytesMut;
    use tokio::codec::Decoder;
    use tokio::codec::Encoder;

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

    pub struct HandshakeCodec {}

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
            let length = src.split_off(1);
            if length != &vec![19] {
                return Err(io::Error::new(io::ErrorKind::Other, "invalid handshake name length"));
            }

            let name = src.split_off(19);
            if name != b"BitTorrent protocol".as_ref() {
                return Err(io::Error::new(io::ErrorKind::Other, "invalid protocol name"));
            }

            let empties = src.split_off(8);
            if empties != [0u8; 8].as_ref() {
                return Err(io::Error::new(io::ErrorKind::Other, "invalid protocol extension"));
            }

            let info_hash = {
                let mut info_hash = [0u8; 20];
                info_hash.copy_from_slice(src.split_off(20).as_ref());
                info_hash
            };

            let peer_id = {
                let mut peer_id = [0u8; 20];
                peer_id.copy_from_slice(src.split_off(20).as_ref());
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
}