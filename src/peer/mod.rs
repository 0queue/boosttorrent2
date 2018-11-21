use std::net::SocketAddr;

use byteorder::{ByteOrder, NetworkEndian};
use futures::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::codec::Framed;
use tokio::net::TcpStream;
use tokio::prelude::{Future, Sink, Stream};

use boostencode::{FromValue, Value};

mod handshake;
pub mod protocol;

#[derive(Debug)]
pub struct PeerInfo {
    pub addr: SocketAddr,
    pub peer_id: Option<[u8; 20]>,
}

pub struct Peer {
    info: PeerInfo,
    pub output: UnboundedSender<u8>,
    pub input: UnboundedReceiver<u8>,
}

pub type Tx<T> = UnboundedSender<T>;
pub type Rx<T> = UnboundedReceiver<T>;

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

    pub fn connect(&self, info_hash: [u8; 20], peer_id: [u8; 20], rt: &mut tokio::runtime::Runtime) -> (Tx<protocol::Message>, Rx<protocol::Message>) {
        // build a future that handshakes a peer
        // then wraps the socket in a peer protocol codec and writes to a channel

        // should find out how to close all these things at some point

        let (input_sender, input_receiver) = mpsc::unbounded();
        let (output_sender, output_receiver) = mpsc::unbounded();

        let peer = TcpStream::connect(&self.addr)
            .and_then(move |socket| {
                let (write, read) = Framed::new(socket, handshake::HandshakeCodec::new()).split();

                write.send((info_hash, peer_id).into())
                    .map(|write| (write, read))
            })
            .and_then(|(write, read)| {
                read.take(1).into_future()
                    .map(|(handshake, read)| (handshake, read.into_inner(), write))
                    .map_err(|(e, _)| e)
            })
            .and_then(move |(maybe_handshake, read, write)| match maybe_handshake {
                Some(ref handshake) if handshake.info_hash == info_hash => futures::future::ok(read.reunite(write).unwrap().into_inner()),
                _ => {
                    println!("invalid handshake");
                    futures::future::err(std::io::Error::new(std::io::ErrorKind::Other, "invalid handshake"))
                }
            })
            .map_err(|e| println!("error: {}", e))
            .and_then(|socket| {
                let (socket_output, socket_input) = Framed::new(socket, protocol::MessageCodec::new()).split();
                println!("valid handshake, reframing");
                let output = input_receiver.forward(socket_output.sink_map_err(|e| println!("socket output error: {}", e)));
                let input = socket_input
                    .map_err(|e| println!("socket receive error: {}", e))
                    .forward(output_sender.sink_map_err(|e| println!("output send error: {}", e)));

                // oof that error type
                output.select2(input).map_err(|e| println!("peer io error"))
            });

        rt.spawn(peer.map(|_| ()));

        (input_sender, output_receiver)
    }
}