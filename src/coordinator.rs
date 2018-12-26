use actix::{
    Actor,
    Addr,
    Context,
    Handler,
    Message,
};
use crate::peer::Peer;
use crate::spawner::Spawner;

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

