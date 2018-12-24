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
use actix::Actor;

mod boostencode;
mod metainfo;
mod tracker;
mod piece;
mod spawner;
mod coordinator;
mod peer;
mod codec;

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
            let tracker = tracker::Tracker::new(peer_id, metainfo.announce, metainfo.info_hash, port).start();
            let coordinator = coordinator::Coordinator::new().start();
            let spawner = spawner::Spawner::listen(coordinator, port);
        });
    } else {
        error!("No torrent file provided");
    }
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