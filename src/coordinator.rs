use actix::{
    Actor,
    Addr,
    Context,
    Handler,
    Message,
    ActorFuture,
    fut::{
        ok,
        wrap_future
    },
    prelude::ContextFutureSpawner,
    Arbiter
};
use futures::future::join_all;
use crate::peer::Peer;
use crate::spawner::{
    NewPeer,
    Spawner
};
use crate::tracker::Tracker;
use crate::listener::Listener;

/// Actor that coordinates peer actions, such as assigning and cancelling pieces, sending Have messages
/// and starting the endgame
pub struct Coordinator {
    peers: Vec<Addr<Peer>>,
    spawner: Addr<Spawner>,
}

impl Coordinator {
    pub fn new(spawner: Addr<Spawner>) -> Self {
        Coordinator {
            peers: Vec::new(),
            spawner
        }
    }

}

impl Actor for Coordinator {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        // We need to start up some peer connections.  wait is a method of trait ContextFutureSpawner.
        // This blocks receiving messages until the future resolves
        let mut spawn_futures = Vec::with_capacity(20);
        for _ in 0..20 {
            spawn_futures.push(self.spawner.send(NewPeer))
        }
        let spawn_fut = join_all(spawn_futures);
        let actor_fut = wrap_future::<_, Self>(spawn_fut);
        actor_fut.map_err(|_,_,_| ()).and_then(|peers, actor, _ctx| {
            match peers.into_iter().collect::<Result<Vec<Addr<Peer>>, ()>>() {
                Ok(mut peers) => actor.peers.append(&mut peers),
                // TODO if any of these connections fail, the whole thing fails.  This seems somewhat
                // fragile.  Should be fixed
                Err(()) => panic!("An error occured getting first batch of peers in coordinator")
            }
            ok(())
        }).wait(ctx)
    }
}

#[derive(Message)]
/// Message for adding a peer actor to the coordinator
pub struct AddPeer(pub Addr<Peer>);

impl Handler<AddPeer> for Coordinator {
    type Result = ();

    fn handle(&mut self, msg: AddPeer, _ctx: &mut Context<Self>) {
        self.peers.push(msg.0);
    }
}

