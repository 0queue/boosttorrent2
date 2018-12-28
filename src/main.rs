use std::{
    fs::File,
    io::Read,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use clap::App;
use clap::load_yaml;
use futures::Future;
use futures::stream::Stream;
use log::{
    debug,
    error,
    Level,
    warn,
};
use rand::prelude::*;
use simple_logger::init_with_level;
use tokio::net::TcpListener;

use crate::boostencode::{FromValue, Value};
use crate::peer::PeerId;
use crate::peer::PeerTx;
use crate::peer::LifecycleEvent;

mod boostencode;
mod metainfo;
mod tracker2;
mod peer;

#[cfg(all(not(feature = "real"), not(feature = "experiment")))]
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

    let coordinator = tracker_info.send_event(&stats, tracker2::Event::Started)
        .map_err(|_| ())
        .map(|res| {
            println!("Started: {:?}", res);

            // eventually spawn a bunch of tasks and channels here

            ()
        })
        .and_then(|_| {
            let (fp, rx) = file_progress::FileProgress::new();

            tokio::spawn(fp);

            rx.take(1).into_future().map(|_| ()).map_err(|_| ())
        })
        .and_then(move |_| {
            tracker_info.send_event(&stats, tracker2::Event::Stopped).map_err(|_| ())
        })
        .map(|res| {
            println!("Stopped: {:?}", res);
        });

    tokio::run(coordinator);
}

#[cfg(feature = "experiment")]
fn main() {
    let peer_info = peer::PeerInfo {
        addr: std::net::SocketAddr::new("127.0.0.1".parse().unwrap(), 8080),
        peer_id: None,
    };

    let mut rt = tokio::runtime::Runtime::new().unwrap();

    let (peer_tx, mut rx) = crossbeam_channel::unbounded();

    let (peer_receiver, peer_future) = peer_info.connect(*b"ThisIsGoodForBitcoin", gen_peer_id(), peer_tx.clone());

    // obviously only drop once no more peers are expected to connect
    // but for this experiment we have to drop it so we can exit after a time;
    drop(peer_tx);

    rt.spawn(peer_future);

    let peer = peer_receiver.wait().unwrap();

    println!("choking");
    peer.choke();

    println!("am interested");
    peer.interested(true);

    std::thread::spawn(move || {
        use futures::Sink;
        println!("Closing in five seconds");
        std::thread::sleep(std::time::Duration::from_secs(5));
        println!("Closing");
        peer.close();
    });

    loop {
        match rx.try_recv() {
            Ok(message) => println!("Received: {:?}", message),
            Err(crossbeam_channel::TryRecvError::Empty) => {
                // nothing
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => break,
        }


        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    println!("waiting for runtime");
    rt.shutdown_on_idle().wait().unwrap();

    println!("done");
}

#[cfg(feature = "real")]
fn main() {
    let mut rt = tokio::runtime::Runtime::new().unwrap();

    // TODO maintain peer state in map
    let mut peer_map: HashMap<PeerId, (PeerTx, ())> = HashMap::new();
    let (peer_lifecycle_sender, peer_lifecycle_receiver) = crossbeam_channel::unbounded();
    let (message_sender, message_receiver) = crossbeam_channel::unbounded();

    let cfg = Configuration {
        info_hash: *b"ThisIsGoodForBitcoin",
        peer_id: gen_peer_id(),
    };

    // 1. TODO reqwest to the tracker

    // 2. TODO initial peer set

    // 3. start listener
    let our_addr = "127.0.0.1:8080".parse().unwrap();
    let listener = TcpListener::bind(&our_addr).unwrap();
    rt.spawn(listener.incoming()
        .map_err(|e| eprintln!("failed to accept socket: {:?}", e))
        .for_each(move |socket| peer::handshake_socket(socket, cfg.clone(), message_sender.clone(), peer_lifecycle_sender.clone())));

    // 4. main loop!

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    ctrlc::set_handler(move || {
        running_clone.store(false, Ordering::SeqCst)
    }).unwrap();

    while running.load(Ordering::SeqCst) {
        // 1. process peer lifecycle
        loop {
            match peer_lifecycle_receiver.try_recv() {
                Ok(LifecycleEvent::Started(peer_id, tx)) => { peer_map.insert(peer_id, (tx, ())); }
                Ok(LifecycleEvent::Stopped(peer_id)) => { peer_map.remove(&peer_id); }
                Err(_) => break,
            }
        }

        // 2. TODO process peer messages

        // 3. TODO dispatch commands to peers
    }

    // 5. TODO clean up peers

    // 6. TODO tell tracker about shutdown

    // shutdown
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

#[derive(Clone)]
pub struct Configuration {
    pub info_hash: [u8; 20],
    pub peer_id: PeerId,
}

mod file_progress {
    use futures::Async;
    use futures::Future;
    use futures::sink::Sink;
    use futures::sync::mpsc;
    use rand::Rng;

    pub struct FileProgress {
        pub progress: f64,
        tx: mpsc::UnboundedSender<()>,
    }

    impl FileProgress {
        pub fn new() -> (FileProgress, mpsc::UnboundedReceiver<()>) {
            let (tx, rx) = mpsc::unbounded();

            let fp = FileProgress {
                progress: 0.0,
                tx,
            };

            (fp, rx)
        }
    }

    impl Future for FileProgress {
        type Item = ();
        type Error = ();

        fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
            if self.progress >= 100.0f64 {
                self.tx.unbounded_send(()).unwrap();
                let _ = self.tx.close();
                return Ok(Async::Ready(()));
            }

            self.progress += if rand::thread_rng().gen_bool(0.1) { 0.1 } else { 0.0 };
            Ok(Async::NotReady)
        }
    }
}