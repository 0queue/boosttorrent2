use actix::{
    Actor,
    ActorFuture,
    Addr,
    Arbiter,
    Context,
    Handler,
    Message,
    AsyncContext,
    msgs::StartActor,
    WrapFuture,
};
use tokio::{
    prelude::Stream,
    net::tcp::{
        TcpStream,
        TcpListener,
    },
    prelude::Future
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
    listen_port: u16,
    peer_thread: Addr<Arbiter>,
}

impl Listener {
    pub fn new(coordinator: Addr<Coordinator>, peer_thread: Addr<Arbiter>, listen_port: u16) -> Self {
        Listener {
            coordinator,
            listen_port,
            peer_thread
        }
    }
}

impl Actor for Listener {
    type Context = actix::Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = SocketAddr::from_str(&format!("0.0.0.0:{}", self.listen_port)).unwrap();
        let listener = TcpListener::bind(&addr).unwrap();
        ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(|st| PeerConnecting(st)));
    }
}

/// This message means a peer is trying to connect to us.  Create a peer actor and send it to the
/// coordinator.
struct PeerConnecting(TcpStream);

impl Message for PeerConnecting {
    type Result = ();
}

impl Handler<PeerConnecting> for Listener {
    type Result = ();

    fn handle(&mut self, msg: PeerConnecting, ctx: &mut Context<Self>) {
        // When a peer tries to connect to us, create a new peer actor in the peer arbiter, and send that actor's address
        // to the Coordinator
        let start_actor_msg = StartActor::new(|ctx| Peer::spawn(msg.0, ctx));
        let spawn_fut = self.peer_thread.send(start_actor_msg)
            .into_actor(self)
            .and_then(|peer: Addr<Peer>, actor, _ctx| actor.coordinator.send(AddPeer(peer)).into_actor(actor))
            .map(|_, _, _| ())
            .map_err(|_, _, _| ());
        // since we are not returning anything, just run the future independently
        ctx.spawn(spawn_fut);



    }
}