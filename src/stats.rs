use actix::{
    Actor,
    Message,
    Handler,
    MessageResult,
    Context
};

#[derive(Clone)]
pub struct Stats {
    pub uploaded: u32,
    pub downloaded: u32,
    pub left: u32,
}

impl Stats {
    pub fn new() -> Self {
        Stats {
            uploaded: 0,
            downloaded: 0,
            left: 0
        }
    }
}

impl Actor for Stats {
    type Context = Context<Self>;
}

#[derive(Message)]
pub enum UpdateStat {
    Uploaded(u32),
    Downloaded(u32),
    Left(u32)
}

impl Handler<UpdateStat> for Stats {
    type Result = ();

    fn handle(&mut self, msg: UpdateStat, _ctx: &mut Context<Self>) {
        match msg {
            UpdateStat::Uploaded(u) => self.uploaded += u,
            UpdateStat::Downloaded(d) => self.downloaded += d,
            UpdateStat::Left(l) => self.left += l
        };
    }
}


/// Message for requesting upload/donwload/left stats
pub struct GetStats;

impl Message for GetStats {
    type Result = Stats;
}

impl Handler<GetStats> for Stats {
    type Result = MessageResult<GetStats>;

    fn handle(&mut self, _msg: GetStats, _ctx: &mut Context<Self>) -> Self::Result {
        MessageResult(self.clone())
    }
}