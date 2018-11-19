use bit_vec::BitVec;
use futures::sync::mpsc::{channel, Receiver, Sender};
use log::{
    error,
    trace,
    warn,
};
use metainfo::MetaInfo;
use peer::Peer;
use piece::Piece;
use replace_with::replace_with;
use std::default::Default;
use std::net::SocketAddr;
use std::ops::Deref;
use std::str::FromStr;
use tokio::{
    io::Error,
    net::{
        tcp::Incoming,
        TcpListener,
    },
    prelude::{
        Async,
        Future,
        future,
        Stream,
        stream,
    },
    spawn,
};
use tracker::{
    Tracker,
    TrackerResponse,
};

/// Type alias for a heap allocated Stream trait object
type BoxedStream<T> = Box<dyn Stream<Item=T, Error=()> + Send>;


/// This is the server that will listen for and spawn peer connections, manage the tracker, and
/// write pieces to the file.  This is "main" for a client
pub struct Server {
    peer_id: [u8; 20],
    info_hash: [u8; 20],
    uploaded: u64,
    uploaded_stream: BoxedStream<u32>,
    downloaded: u64,
    downloaded_stream: BoxedStream<u32>,
    left: u64,
    listener: Incoming,
    tracker: Tracker,
    piece_stream: BoxedStream<(Piece, Sender<Piece>, BitVec)>,
}

impl Server {
    pub fn new(peer_id: [u8; 20], meta: MetaInfo) -> Self {
        let address = SocketAddr::from_str("0.0.0.0:6888").unwrap();
        let download_size = meta.info.file_info.size() as u64;
        let mut tracker = Tracker::new(
            peer_id.clone(),
            meta.announce,
            meta.info_hash.clone(),
            6888,
        );
        let info_hash = meta.info_hash;
        tracker.start(download_size);
        Server {
            peer_id,
            info_hash,
            uploaded: 0,
            uploaded_stream: Box::new(stream::empty()),
            downloaded: 0,
            downloaded_stream: Box::new(stream::empty()),
            left: download_size,
            listener: TcpListener::bind(&address).expect("Failed to open TCP listener").incoming(),
            tracker,
            piece_stream: Box::new(stream::empty()),
        }
    }
}

impl Future for Server {
    type Item = ();
    type Error = ();

    /// This is the main event loop for the client.  It returns Ok(Ready(())) Only when the download
    /// is complete.
    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        trace!("Start Loop");
        // check on the tracker response
        match self.tracker.poll() {
            Err(e) => {
                error!("Something went wrong in making a request to the tracker: {:?}", e);
                return Err(());
            }
            Ok(Async::Ready(TrackerResponse::Failure(msg))) => error!("The tracker responded with an error: {}", msg),
            Ok(Async::Ready(TrackerResponse::Warning(msg, resp))) => {
                warn!("The tracker responeded with a warning: {}", msg);
                // do something with the response
                trace!("tracker response: {:?}", resp)
            }
            Ok(Async::Ready(TrackerResponse::Success(resp))) => {
                // do something with the response
                trace!("tracker response: {:?}", resp)
            }
            _ => () // not ready
        };
        // poll for new connections, spin up new peer tasks
        loop {
            match self.listener.poll() {
                Ok(Async::Ready(Some(conn))) => {
                    let (up_sender, up_receiver) = channel(10);
                    let (down_sender, down_receiver) = channel(10);
                    let (piece_sender, piece_receiver) = channel(10);

                    replace_with(&mut self.uploaded_stream,
                                 /* default, in case replacement panics */ || Box::new(stream::empty()),
                                 |s| Box::new(s.select(up_receiver)));
                    replace_with(&mut self.downloaded_stream,
                                 || Box::new(stream::empty()),
                                 |s| Box::new(s.select(down_receiver)));
                    replace_with(&mut self.piece_stream,
                                 || Box::new(stream::empty()),
                                 |s| Box::new(s.select(piece_receiver)));
                    let peer = Peer::new(conn,
                                         up_sender,
                                         down_sender,
                                         piece_sender,
                                         self.info_hash.clone(),
                                         self.peer_id.clone(),
                                         false);
                    spawn(peer);
                }
                Err(e) => {
                    error!("TCP Listener closed unexpectedly with error: {}", e);
                    return Err(());
                }
                _ => break,
            }
        }

        // get uploaded/downloaded statistic updates
        loop {
            match self.uploaded_stream.poll() {
                Ok(Async::Ready(Some(update))) => self.uploaded += update as u64,
                _ => break,
            }
        }
        loop {
            match self.downloaded_stream.poll() {
                Ok(Async::Ready(Some(update))) => self.downloaded += update as u64,
                _ => break,
            }
        }

        // Get finished pieces and request new pieces
        loop {
            match self.piece_stream.poll() {
                Ok(Async::Ready(Some((finished_piece, new_piece_sender, availible_pieces)))) => {
                    // TODO verify and write off the finished piece and either kill the peer or give
                    // them a new piece
                }
                _ => break
            }
        }

        // This future only finishes normally when the download is complete
        if self.left == 0 {
            trace!("Finished");
            Ok(Async::Ready(()))
        } else {
            trace!("Did a loop");
            Ok(Async::NotReady)
        }
    }
}