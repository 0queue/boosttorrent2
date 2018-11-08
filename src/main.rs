#[warn(unused_extern_crates)]
extern crate clap;
extern crate crypto;
extern crate derive_error;
extern crate futures;
extern crate log;
extern crate maplit;
extern crate rand;
extern crate reqwest;
extern crate serde;
extern crate simple_logger;
extern crate tokio;
extern crate byteorder;

use std::fs::File;
use std::io::Read;

use clap::App;
use clap::load_yaml;
use futures::Future;
use log::{
    debug,
    error,
    Level,
    warn,
};
use rand::prelude::*;
use simple_logger::init_with_level;

use boostencode::{FromValue, Value};

mod boostencode;
mod metainfo;
mod tracker2;
mod peer;

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

    if !matches.is_present("torrent-file") {
        error!("No torrent file provided");
        return;
    }

    let string = matches.value_of("torrent-file").unwrap();
    let mut f = File::open(string).expect("file not found");
    let mut contents = Vec::new();
    f.read_to_end(&mut contents).expect("error reading file");
    let val = Value::decode(contents.as_ref()).unwrap();
    debug!("{}", val);

    let metainfo = metainfo::MetaInfo::from_value(&val).unwrap();

    let peer_id = gen_peer_id();


    let stats = tracker2::Stats {
        uploaded: 0,
        downloaded: 0,
        left: metainfo.info.file_info.size() as u64,
    };

    let addr = reqwest::Url::parse(&metainfo.announce).unwrap();
    let tracker_info = tracker2::Tracker::new(addr, metainfo.info_hash, peer_id, 6881);

    let interaction = tracker_info.send_event(&stats, tracker2::Event::Started)
        .and_then(|response| {
            println!("{:?}", response);
            Ok(())
        });

    tokio::run(interaction);
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