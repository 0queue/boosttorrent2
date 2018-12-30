use actix::{
    Actor,
    Arbiter,
    Addr,
    Context,
    Handler,
    Message,
    ResponseActFuture,
    WrapFuture,
    ActorFuture,
    fut::{
        ok,
        err,
        wrap_future,
    },
    msgs::StartActor,
};
use tokio::{
    prelude::Future,
    net::tcp::TcpStream,
};
use crate::peer::Peer;
use crate::tracker::{
    Event,
    PeerInfo,
    Tracker,
    TrackerResponse,
};
use crate::coordinator::Coordinator;

pub struct Spawner {
    tracker: Addr<Tracker>,
    coordinator: Addr<Coordinator>,
    potential_peers: Vec<PeerInfo>,
    peer_thread: Addr<Arbiter>,
}

impl Actor for Spawner {
    type Context = actix::Context<Self>;
}

impl Spawner {
    pub fn new(tracker: Addr<Tracker>, coordinator: Addr<Coordinator>, peer_thread: Addr<Arbiter>) -> Self {
        Spawner {
            tracker,
            coordinator,
            potential_peers: Vec::new(),
            peer_thread,
        }
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
                .and_then(|resp, actor, _ctx| {
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
                        .map_err(|_| ()))
                })
                .and_then(|stream, actor, _| {
                    // wrap the tcp stream into a peer and send it off to the peer arbiter to run on
                    let start_actor_msg = StartActor::new(|ctx| Peer::spawn(stream, ctx));
                    actor.peer_thread.send(start_actor_msg)
                        .map_err(|_| ())
                        .into_actor(actor)

                }))
        } else {
            Box::new(wrap_future::<_, Self>(TcpStream::connect(&self.potential_peers.remove(0).address)
                .map_err(|_| ()))
                .and_then(|stream, actor, _| {
                    // wrap the tcp stream into a peer and send it off to the peer arbiter to run on
                    let start_actor_msg = StartActor::new(|ctx| Peer::spawn(stream, ctx));
                    actor.peer_thread.send(start_actor_msg)
                        .map_err(|_| ())
                        .into_actor(actor)
                }))
        }
    }
}