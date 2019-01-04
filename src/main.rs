use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crossbeam_channel::select;
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use lazy_static::lazy_static;
use log::Level;
use rand::prelude::*;
use simple_logger::init_with_level;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

use crate::boostencode::{FromValue, Value};
use crate::metainfo::MetaInfo;
use crate::peer::LifecycleEvent;
use crate::peer::PeerId;
use crate::peer::PeerMessageExt;
use crate::peer::PeerTx;
use crate::peer::pretty;

mod boostencode;
mod metainfo;
mod tracker;
mod peer;

#[derive(Clone)]
pub struct Configuration {
    pub info_hash: [u8; 20],
    pub peer_id: PeerId,
    pub garbage_mode: bool,
    pub metainfo: Option<MetaInfo>,
    pub test_connect: Option<SocketAddr>,
}

lazy_static! {
    static ref CONFIG: Configuration = parse_cfg();
}

fn main() {
    let mut rt = tokio::runtime::Runtime::new().unwrap();

    // TODO maintain peer state in map
    let mut peer_map: HashMap<PeerId, (PeerTx, ())> = HashMap::new();
    let (peer_lifecycle_sender, peer_lifecycle_receiver) = crossbeam_channel::unbounded();
    let (message_sender, message_receiver) = crossbeam_channel::unbounded();

    // 1. TODO reqwest to the tracker

    // 2. TODO initial peer set

    // borrow checker did not like this for some reason, after cleaning up at the bottom
    // instead of thinking about it more, I'll just clone everything
    // which is valid for at least the channels, but I don't know the deal with cfg
    if CONFIG.test_connect.is_some() {
        let msg_send_clone = message_sender.clone();
        let lifecycle_clone = peer_lifecycle_sender.clone();
        rt.spawn(TcpStream::connect(&CONFIG.test_connect.unwrap())
            .map_err(|e| eprintln!("Test connect error: {:?}", e))
            .and_then(move |socket| {
                peer::handshake_socket(socket, &CONFIG, msg_send_clone, lifecycle_clone)
            }));
    }

    // 3. start listener
    let our_addr = "127.0.0.1:8080".parse().unwrap();

    let listener = TcpListener::bind(&our_addr).unwrap().incoming()
        .map_err(|e| eprintln!("failed to accept socket: {:?}", e))
        .for_each(move |socket| peer::handshake_socket(socket, &CONFIG, message_sender.clone(), peer_lifecycle_sender.clone()));
    let (shutdown_sender, shutdown_receiver) = futures::oneshot();

    // wow aren't integrated error types great
    rt.spawn(listener.select2(shutdown_receiver.map_err(|_| ())).map(|_| ()).map_err(|_| ()));

    // 4. main loop!

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    ctrlc::set_handler(move || {
        running_clone.store(false, Ordering::SeqCst)
    }).unwrap();

    println!("Entering main loop");
    while running.load(Ordering::SeqCst) {
        select! {
            // process peers connecting or disconnecting
            recv(peer_lifecycle_receiver) -> lifecycle_event => match lifecycle_event {
                Ok(LifecycleEvent::Started(peer_id, tx)) => {
                    println!("New peer: {}", pretty(&peer_id));
                    peer_map.insert(peer_id, (tx, ()));


                    // nice it works
                    if CONFIG.test_connect.is_some() {
                        peer_map.get(&peer_id).map(|(tx, _)| tx.choke());
                        peer_map.get(&peer_id).map(|(tx, _)| tx.interested(true));
                    }
                }
                Ok(LifecycleEvent::Stopped(peer_id)) => {
                    println!("Removing peer: {}", pretty(&peer_id));
                    peer_map.remove(&peer_id);
                }
                Err(_) => break,
            },
            // process peer messages
            recv(message_receiver) -> msg => match msg {
                Ok((peer_id, message)) => {
                    println!("From {}: {:?}", pretty(&peer_id), message);
                }
                Err(_) => break,
            },
            // if nothing to do, make sure all peers are busy
            default => {
                // TODO dispatch commands
            }
        }
    }

    println!("Cleaning up");

    // 5. clean up peers
    for (_peer_id, (mut tx, _)) in peer_map {
        tx.close();
    }

    // 6. TODO tell tracker about shutdown

    // 7. shutdown
    shutdown_sender.send(());
    rt.shutdown_on_idle().wait().unwrap();
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

fn parse_cfg() -> Configuration {
    let yaml = clap::load_yaml!("cli.yml");
    let matches = clap::App::from_yaml(yaml).get_matches();

    match matches.occurrences_of("verbose") {
        0 => init_with_level(Level::Error),
        1 => init_with_level(Level::Warn),
        2 => init_with_level(Level::Info),
        3 => init_with_level(Level::Debug),
        4 | _ => init_with_level(Level::Trace),
    }.unwrap();

    let (info_hash, metainfo) = {
        let maybe_filename = matches.value_of("torrent-file");
        let maybe_metainfo = maybe_filename.map(|filename| {
            let mut f = File::open(filename).expect("file not found");
            let mut contents = Vec::new();
            f.read_to_end(&mut contents).expect("error reading file");
            let val = Value::decode(&contents).unwrap();
            MetaInfo::from_value(&val).unwrap()
        });

        let info_hash = match maybe_metainfo {
            Some(ref m) => m.info_hash,
            None => *b"ThisIsGoodForBitcoin"
        };

        (info_hash, maybe_metainfo)
    };

    let test_connect = {
        let maybe_addr = matches.value_of("test-connect");
        maybe_addr.map(|s| s.parse().unwrap())
    };

    Configuration {
        info_hash,
        peer_id: gen_peer_id(),
        garbage_mode: matches.is_present("garbage-mode"),
        metainfo,
        test_connect,
    }
}