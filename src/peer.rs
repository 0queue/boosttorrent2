use actix::{
    Actor,
    Addr,
    AsyncContext,
    Context,
    Handler,
    io::{
        FramedWrite,
        WriteHandler
    }
};
use tokio::{
    net::tcp::TcpStream,
    io::{
        AsyncRead,
        WriteHalf
    },
    codec::FramedRead,
    prelude::{
        Stream
    }
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

    pub fn spawn(stream: TcpStream) -> Addr<Peer> {
        let (reader, writer) = stream.split();
        Self::create(|ctx| {
            ctx.add_message_stream(FramedRead::new(reader, MessageCodec::new()).map_err(|_| ()));
            Peer::new(FramedWrite::new(writer, MessageCodec::new(), ctx))
        })
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