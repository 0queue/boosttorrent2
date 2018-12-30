use crate::boostencode::{FromValue, Value};
use clap::App;
use clap::load_yaml;
use log::{
    debug,
    error,
    Level,
    warn,
};
use rand::prelude::*;
use simple_logger::init_with_level;
use std::fs::File;
use std::io::Read;
use actix::{
    Actor,
    Addr,
    Arbiter,
    AsyncContext,
    Context
};

mod boostencode;
mod metainfo;
mod tracker;
mod piece;
mod listener;
mod coordinator;
mod peer;
mod codec;
mod spawner;
mod stats;

fn main() {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    match matches.occurrences_of("verbose") {
        0 => init_with_level(Level::Error),
        1 => init_with_level(Level::Warn),
        2 => init_with_level(Level::Info),
        3 => init_with_level(Level::Debug),
        4 | _ => init_with_level(Level::Trace),
    }.unwrap();

    if matches.is_present("garbage-mode") {
        warn!("Garbage mode activated");
    }

    if matches.is_present("torrent-file") {
        let string = matches.value_of("torrent-file").unwrap();
        let mut f = File::open(string).expect("file not found");
        let mut contents = Vec::new();
        f.read_to_end(&mut contents).expect("error reading file");
        let val = Value::decode(contents.as_ref()).unwrap();
        debug!("{}", val);

        let metainfo = metainfo::MetaInfo::from_value(&val).unwrap();
        debug!("{:?}", metainfo);

        let peer_id = gen_peer_id();
        let port = 6888;
        actix::System::run(move || {
            let tracker = start_misc_thread(peer_id, metainfo.announce.clone(), metainfo.info_hash.clone(), port);
            // tell the tracker to make a request.  That request will be cached for subsequent refreshes,
            // so we don't need to store the result now
            tracker.do_send(tracker::Event::Start);
            // We don't start this arbiter with anything. Instead we pass it to the spawner/listener
            // which will spawn new peers in it.
            let peer_thread = Arbiter::new("peer thread");
            start_coordinator_thread(tracker, peer_thread, port);
        });
    } else {
        error!("No torrent file provided");
    }
}

/// Starts the coordinator, spawner, and listener on the same thread (I think, the docs aren't clear on this,
/// but as far as I can tell, Actor::start starts the actor in the current arbiter)
fn start_coordinator_thread(tracker: Addr<tracker::Tracker>, peer_thread: Addr<Arbiter>, listen_port: u16) {
    Arbiter::start(move |ctx: &mut Context<coordinator::Coordinator>| {
        let _listener = listener::Listener::new(ctx.address(), peer_thread.clone(), listen_port).start();
        let spawner = spawner::Spawner::new(tracker, ctx.address(), peer_thread).start();
        coordinator::Coordinator::new(spawner)
    });
}

/// Starts a thread on which miscalaneous low-priority actors can run on and returns those actors
fn start_misc_thread(peer_id: [u8; 20], announce: String, info_hash: [u8; 20], port: u16) -> Addr<tracker::Tracker> {
    Arbiter::start(move |_ctx: &mut Context<tracker::Tracker>| {
        let stats = stats::Stats::new().start();
        tracker::Tracker::new(stats.clone(),
                              peer_id,
                              announce,
                              info_hash,
                              port)


    })
}

fn gen_peer_id() -> [u8; 20] {
    // Generate peer id in Azures style ("-<2 letter client code><4 digit version number>-<12 random digits>")
    let mut id = "-BO0001-".to_owned();
    for _ in 0..12 {
        id.push_str(&thread_rng().gen_range::<u8>(0, 9).to_string());
    }
    let mut res = [0; 20];
    res.copy_from_slice(id.as_bytes());
    res
}