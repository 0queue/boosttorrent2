use actix::{
    Actor,
    Addr,
    Context,
    Handler,
    Message,
    io::FramedWrite,
    AsyncContext,
    ResponseActFuture,
    WrapFuture,
    ActorFuture,
    fut::{
        ok,
        err,
        wrap_future,
    },
};
use tokio::{
    prelude::{
        Future,
        Stream,
    },
    net::tcp::{
        TcpStream,
        TcpListener,
    },
    codec::FramedRead,
    io::AsyncRead,
};
use std::net::SocketAddr;
use std::str::FromStr;
use crate::coordinator::{
    Coordinator,
    AddPeer,
};
use crate::peer::Peer;
use crate::codec::MessageCodec;
use crate::tracker::{
    Event,
    PeerInfo,
    Tracker,
    TrackerResponse,
    TrackerSuccessResponse,
};

/// Actor that will spawn connections to peers
pub struct Spawner {
    coordinator: Addr<Coordinator>,
    tracker: Addr<Tracker>,
    potential_peers: Vec<PeerInfo>,
}

impl Spawner {
    pub fn listen(coordinator: Addr<Coordinator>, tracker: Addr<Tracker>, port: u16) -> Addr<Self> {
        Self::create(move |ctx| {
            let addr = SocketAddr::from_str(&format!("0.0.0.0:{}", port)).unwrap();
            let listener = TcpListener::bind(&addr).unwrap();
            ctx.add_message_stream(listener.incoming().map_err(|_| ()).map(|st| PeerConnecting(st)));
            Spawner {
                coordinator,
                tracker,
                potential_peers: Vec::new(),
            }
        })
    }

    fn spawn_peer(stream: TcpStream) -> Addr<Peer> {
        let (reader, writer) = stream.split();
        Peer::create(|ctx| {
            ctx.add_message_stream(FramedRead::new(reader, MessageCodec::new()).map_err(|_| ()));
            Peer::new(FramedWrite::new(writer, MessageCodec::new(), ctx))
        })
    }
}

impl Actor for Spawner {
    type Context = actix::Context<Self>;
}

/// This message means a peer is trying to connect to us.  Create a peer actor and send it to the
/// coordinator.
struct PeerConnecting(TcpStream);

impl Message for PeerConnecting {
    type Result = ();
}

impl Handler<PeerConnecting> for Spawner {
    type Result = ();

    fn handle(&mut self, msg: PeerConnecting, _ctx: &mut Context<Self>) {
        // When a peer tries to connect to us, create a new peer actor, and send that actor's address
        // to the Coordinator
        let peer = Self::spawn_peer(msg.0);
        self.coordinator.do_send(AddPeer(peer));
    }
}

/// This message is a request to connect to a new peer
pub struct NewPeer;

impl Message for NewPeer {
    type Result = Result<Addr<Peer>, ()>;
}

impl Handler<NewPeer> for Spawner {
    type Result = ResponseActFuture<Self, Addr<Peer>, ()>;

    fn handle(&mut self, _msg: NewPeer, _ctx: &mut Context<Self>) -> Self::Result {
        // create a future that returns a PeerInfo.  If we have some cached, then this is simple.
        // If not, we need to message the tracker for that info, and update our own state with the
        // result
        if self.potential_peers.is_empty() {
            // Box because ResponseActFuture is a type alias for Box<ActorFuture>
            Box::new(self.tracker.send(Event::Refresh)
                .map_err(|_| ())
                // into_actor so we can mutate our state when the tracker returns and cache the returned peers
                .into_actor(self)
                .and_then(|resp, actor, ctx| {
                    match resp {
                        Ok(TrackerResponse::Warning(_, mut resp)) | Ok(TrackerResponse::Success(mut resp)) => {
                            actor.potential_peers.append(&mut resp.peers);
                            ok(actor.potential_peers.remove(0))
                        }
                        _ => err(())
                    }
                })
                .and_then(|peer_info, _, _| {
                    // wrap_future to turn this from a regular future into an actor future
                    wrap_future(TcpStream::connect(&peer_info.address)
                        .map_err(|_| ())
                        .map(|stream| {
                            Self::spawn_peer(stream)
                        }))
                }))
        } else {
            Box::new(wrap_future(TcpStream::connect(&self.potential_peers.remove(0).address)
                .map_err(|_| ())
                .map(|stream| {
                    Self::spawn_peer(stream)
                })))
        }
    }
}