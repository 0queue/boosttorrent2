use actix::{
    Actor,
    Addr,
    Context,
    Handler,
    Message,
    io::FramedWrite,
    AsyncContext
};
use tokio::{
    prelude::{
        Stream,
    },
    net::tcp::{
        TcpStream,
        TcpListener
    },
    codec::{
        FramedRead,
    },
    io::AsyncRead
};
use std::net::SocketAddr;
use std::str::FromStr;
use crate::coordinator::{
    Coordinator,
    AddPeer
};
use crate::peer::Peer;
use crate::codec::MessageCodec;

/// Actor that will spawn connections to peers
pub struct Spawner {
    coordinator: Addr<Coordinator>
}

impl Spawner {
    pub fn listen(coordinator: Addr<Coordinator>, port: u16) -> Addr<Self> {
        Self::create(move |ctx| {
            let addr = SocketAddr::from_str(&format!("0.0.0.0:{}", port)).unwrap();
            let listener = TcpListener::bind(&addr).unwrap();
            ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(|st| PeerConnecting(st)));
            Spawner {
                coordinator
            }
        })
    }
}

impl Actor for Spawner {
    type Context = actix::Context<Self>;
}

/// This message means a peer is trying to connect to us
struct PeerConnecting(TcpStream);

impl Message for PeerConnecting {
    type Result = ();
}

impl Handler<PeerConnecting> for Spawner {
    type Result = ();

    fn handle(&mut self, msg: PeerConnecting, _ctx: &mut Context<Self>) {
        // When a peer tries to connect to us, create a new peer actor, and send that actor's address
        // to the Coordinator
        let (reader, writer) = msg.0.split();
        let peer = Peer::create(|ctx| {
            ctx.add_message_stream(FramedRead::new(reader, MessageCodec::new()).map_err(|_| ()));
            Peer::new(FramedWrite::new(writer, MessageCodec::new(), ctx))
        });
        self.coordinator.do_send(AddPeer(peer));
    }
}

