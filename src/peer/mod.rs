use std::io::Error;
use std::io::ErrorKind;
use std::net::SocketAddr;

use byteorder::{ByteOrder, NetworkEndian};
use crossbeam_channel::Sender;
use futures::sync::mpsc::{self, UnboundedSender};
use tokio::codec::Framed;
use tokio::prelude::{Future, Sink, Stream};

use crate::boostencode::Value;
use crate::peer::channel_sink::ChannelSink;
use crate::peer::protocol::Message;

mod handshake;
mod channel_sink;
pub mod protocol;

pub type PeerId = [u8; 20];
pub type PeerTx = UnboundedSender<Message>;

pub enum LifecycleEvent {
    Started(PeerId, PeerTx),
    Stopped(PeerId),
}

/// information from the tracker
#[derive(Debug)]
pub struct PeerInfo {
    pub addr: SocketAddr,
    pub peer_id: Option<PeerId>,
}

impl crate::boostencode::FromValue for PeerInfo {
    type Error = &'static str;

    fn from_value(val: &Value) -> Result<Self, Self::Error> where Self: Sized {
        let map = val.dict().ok_or("Not a dictionary")?;

        let mut peer_id = [0u8; 20];
        map.get("peer id".as_bytes()).and_then(Value::bstring)
            .map(|bytes| peer_id.copy_from_slice(bytes))
            .ok_or("Missing key: peer id")?;
        let peer_id = Some(peer_id);

        let ip = map.get("ip".as_bytes()).and_then(Value::bstring_utf8)
            .map(|s| s.parse())
            .ok_or("Missing key: ip")?
            .map_err(|_| "Invalid ip addr")?;

        let port = map.get("port".as_bytes()).and_then(Value::integer)
            .map(|i| *i as u16)
            .ok_or("Missing key: port")?;

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

    pub fn connect(&self) -> tokio::net::tcp::ConnectFuture {
        tokio::net::TcpStream::connect(&self.addr)
    }
}

pub trait PeerMessageExt {
    fn choke(&self);

    fn interested(&self, is_interested: bool);

    // TODO more
}

impl PeerMessageExt for UnboundedSender<Message> {
    fn choke(&self) {
        self.unbounded_send(Message::Choke).unwrap()
    }

    fn interested(&self, is_interested: bool) {
        self.unbounded_send(if is_interested {
            Message::Interested
        } else {
            Message::NotInterested
        }).unwrap()
    }
}

pub fn handshake_socket(
    socket: tokio::net::TcpStream,
    cfg: crate::Configuration,
    output_message_sender: Sender<(PeerId, Message)>,
    lifecycle_sender: Sender<LifecycleEvent>,
) -> impl Future<Item=(), Error=()> {
    let (input_message_sender, input_message_receiver) = mpsc::unbounded();

    let (write, read) = Framed::new(socket, handshake::HandshakeCodec::new()).split();

    write.send((cfg.info_hash, cfg.peer_id).into())
        .and_then(|write| {
            read.take(1).into_future()
                .map(|(handshake, read)| (handshake, read.into_inner(), write))
                .map_err(|(e, _)| e)
        })
        .and_then(move |(maybe_handshake, read, write)| match maybe_handshake {
            Some(ref handshake) if handshake.info_hash == cfg.info_hash =>
                futures::future::ok((handshake.peer_id, read.reunite(write).unwrap().into_inner())),
            _ => futures::future::err(Error::new(ErrorKind::Other, "invalid handshake"))
        })
        .map_err(|e| eprintln!("Error while handshaking: {:?}", e))
        .and_then(move |(their_peer_id, socket)| {
            let _ = lifecycle_sender.send(LifecycleEvent::Started(their_peer_id, input_message_sender));

            let (socket_output, socket_input) = Framed::new(socket, protocol::MessageCodec::new()).split();

            let output = input_message_receiver.forward(socket_output.sink_map_err(|e| eprintln!("Socket output error: {:?}", e)));
            let input = socket_input
                .map_err(|e| eprintln!("Socket input error: {:?}", e))
                .map(move |msg| (their_peer_id, msg))
                .forward(ChannelSink::new(output_message_sender).sink_map_err(|e| eprintln!("error forwarding socket input: {:?}", e)));

            output.select2(input)
                .map_err(|_| eprintln!("peer io error"))
                .then(move |_| {
                    let _ = lifecycle_sender.send(LifecycleEvent::Stopped(their_peer_id));
                    Ok(())
                })
        })
}

/// For debugging purposes only
/// because why bother with implementing
/// someone else's trait for newtype
pub fn pretty(peer_id: &PeerId) -> String {
    use std::fmt::Write;

    let mut s = String::new();
    for byte in peer_id {
        write!(s, "{:02x}", byte).unwrap();
    }
    s
}