use std::io::Read;
use std::io::Write;
use std::net::SocketAddr;

use byteorder::{ByteOrder, NetworkEndian};
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::codec::Framed;
use tokio::net::TcpStream;
use tokio::prelude::{Future, Sink, Stream};

use boostencode::{FromValue, Value};

mod handshake;
mod protocol;

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

    pub fn connect(&self, info_hash: [u8; 20], peer_id: [u8; 20]) -> () {
        // build a future that handshakes a peer
        // then wraps the socket in a peer protocol codec and writes to a channel

        TcpStream::connect(&self.addr).and_then(|socket| {
            let (write, read) = Framed::new(socket, handshake::HandshakeCodec::new()).split();

            write.send((info_hash, peer_id).into())
                .and_then(|write| {
                    read.take(1).into_future()
                        .map(|(h, r)| (h, r.into_inner(), write))
                        .map_err(|(e, _)| e)
                })
                .and_then(|(maybe_handshake, read, write)| match maybe_handshake {
                    Some(ref handshake) if handshake.info_hash == info_hash => Ok(read.reunite(write).unwrap().into_inner()),
                    _ => Err(std::io::Error::new(std::io::ErrorKind::Other, "invalid handshake"))
                })
                .and_then(|socket| {
                    // protocol codec time
                    Ok(())
                })
        });

        ()
    }
}