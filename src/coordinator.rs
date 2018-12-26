use actix::{
    Actor,
    Addr,
    Context,
    Handler,
    Message,
    MessageResult,
    dev::MessageResponse
};
use crate::peer::Peer;

/// Actor that coordinates peer actions, such as assigning and cancelling pieces, sending Have messages
/// and starting the endgame
pub struct Coordinator {
    peers: Vec<Addr<Peer>>,
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
/// Message for adding a peer actor to the coordinator
pub struct AddPeer(pub Addr<Peer>);

impl Handler<AddPeer> for Coordinator {
    type Result = ();

    fn handle(&mut self, msg: AddPeer, _ctx: &mut Context<Self>) {
        self.peers.push(msg.0);
    }
}

/// Message for requesting upload/donwload/left stats
pub struct GetStats;

pub struct Stats {
    pub uploaded: u32,
    pub downloaded: u32,
    pub left: u32,
}

impl Message for GetStats {
    type Result = Stats;
}

impl Handler<GetStats> for Coordinator {
    type Result = MessageResult<GetStats>;

    fn handle(&mut self, _msg: GetStats, _ctx: &mut Context<Self>) -> Self::Result {
        MessageResult(Stats {
            uploaded: 0,
            downloaded: 0,
            left: 0,
        })
    }
}