use actix::{
    Actor,
    Addr,
    Context,
    Handler,
    io::{
        FramedWrite,
        WriteHandler
    }
};
use tokio::{
    net::tcp::TcpStream,
    io::WriteHalf
};
use crate::codec::{
    BitTorrentMessage,
    MessageCodec
};

/// Actor that communicates with a network peer to upload and download pieces
pub struct Peer {
    writer: FramedWrite<WriteHalf<TcpStream>, MessageCodec>,
}

impl Peer {
    pub fn new(writer: FramedWrite<WriteHalf<TcpStream>, MessageCodec>) -> Self {
        Peer {
            writer
        }
    }
}

impl Actor for Peer {
    type Context = Context<Self>;
}

// Necessary for FramedWrite to work
impl WriteHandler<::std::io::Error> for Peer {}

impl Handler<BitTorrentMessage> for Peer {
    type Result = ();

    fn handle(&mut self, msg: BitTorrentMessage, _ctx: &mut Context<Self>) {

    }
}