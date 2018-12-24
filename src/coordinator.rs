use actix::{
    Actor,
    Addr,
    Context,
    Handler,
    Message
};
use crate::peer::Peer;

/// Actor that coordinates peer actions, such as assigning and cancelling pieces, sending Have messages
/// and starting the endgame
pub struct Coordinator {
    peers: Vec<Addr<Peer>>
}

impl Coordinator {
    pub fn new() -> Self {
        Coordinator {
            peers: Vec::new()
        }
    }
}

impl Actor for Coordinator {
    type Context = Context<Self>;
}

#[derive(Message)]
pub struct AddPeer(pub Addr<Peer>);

impl Handler<AddPeer> for Coordinator {
    type Result = ();

    fn handle(&mut self, msg: AddPeer, _ctx: &mut Context<Self>) {
        self.peers.push(msg.0);
    }
}