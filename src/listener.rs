use actix::{
    Actor,
    Addr,
    Context,
    Handler,
    Message,
    AsyncContext,
};
use tokio::{
    prelude::{
        Stream,
    },
    net::tcp::{
        TcpStream,
        TcpListener,
    },
};
use std::net::SocketAddr;
use std::str::FromStr;
use crate::coordinator::{
    Coordinator,
    AddPeer,
};
use crate::peer::Peer;


/// Actor that will spawn connections to peers
pub struct Listener {
    coordinator: Addr<Coordinator>,
}

impl Listener {
    pub fn listen(coordinator: Addr<Coordinator>, port: u16) -> Addr<Self> {
        Self::create(move |ctx| {
            let addr = SocketAddr::from_str(&format!("0.0.0.0:{}", port)).unwrap();
            let listener = TcpListener::bind(&addr).unwrap();
            ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(|st| PeerConnecting(st)));
            Listener {
                coordinator,
            }
        })
    }
}

impl Actor for Listener {
    type Context = actix::Context<Self>;
}

/// This message means a peer is trying to connect to us.  Create a peer actor and send it to the
/// coordinator.
struct PeerConnecting(TcpStream);

impl Message for PeerConnecting {
    type Result = ();
}

impl Handler<PeerConnecting> for Listener {
    type Result = ();

    fn handle(&mut self, msg: PeerConnecting, _ctx: &mut Context<Self>) {
        // When a peer tries to connect to us, create a new peer actor, and send that actor's address
        // to the Coordinator
        let peer = Peer::spawn(msg.0);
        self.coordinator.do_send(AddPeer(peer));
    }
}