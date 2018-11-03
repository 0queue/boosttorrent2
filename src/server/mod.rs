use log::{
    error,
    trace,
    warn,
};
use metainfo::MetaInfo;
use std::default::Default;
use std::net::SocketAddr;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::{
    Arc,
    RwLock,
};
use tokio::{
    net::TcpListener,
    prelude::{
        Async,
        Future,
        Stream,
    },
    io::Error,
};
use tracker::{
    Tracker,
    TrackerResponse,
};

/// This struct contains all state that is shared between tasks. This struct should not be created
/// outside of this module.
#[derive(Debug)]
pub struct State {
    pub peer_id: [u8; 20],
    pub uploaded: u32,
    pub downloaded: u32,
    pub left: u32,
}

/// This type is a synchronized wrapper around State.  This is what other modules will use
#[derive(Debug, Clone)]
pub struct SharedState(Arc<RwLock<State>>);

impl Default for SharedState {
    fn default() -> Self {
        SharedState(Arc::new(RwLock::new(State {
            peer_id: [0; 20],
            uploaded: 0,
            downloaded: 0,
            left: 0,
        })))
    }
}

impl Deref for SharedState {
    type Target = Arc<RwLock<State>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// This is the server that will listen for and spawn peer connections, manage the tracker, and
/// write pieces to the file.  This is "main" for a client
pub struct Server {
    listener: Box<dyn Future<Item=(), Error=Error> + Send>,
    state: SharedState,
    tracker: Tracker,

}

impl Server {
    pub fn new(peer_id: [u8; 20], meta: MetaInfo) -> Self {
        let s = State {
            peer_id,
            uploaded: 0,
            downloaded: 0,
            left: meta.info.file_info.size() as u32,
        };
        let state = SharedState(Arc::new(RwLock::new(s)));
        let address = SocketAddr::from_str("0.0.0.0:6888").unwrap();
        let listener = Box::new(TcpListener::bind(&address)
            .expect("Failed to open TCP listener")
            .incoming()
            .for_each(|peer| {
                // create a new future to handle this peer.  For now drop
                trace!("peer connection: {:?}", peer);
                Ok(())
            }));
        let mut tracker = Tracker::new(
            meta.announce,
            meta.info_hash,
            6888,
            state.clone(),
        );
        tracker.start();
        Server {
            listener,
            state,
            tracker,
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
        // poll for new connections
        match self.listener.poll() {
            Err(e) => {
                error!("The TCP listener encountered an error: {:?}", e);
                return Err(());
            }
            _ => ()
        };

        // This future only finishes normally when the download is complete
        if self.state.read().unwrap().left == 0 {
            trace!("Finished");
            Ok(Async::Ready(()))
        } else {
            trace!("Did a loop");
            Ok(Async::NotReady)
        }
    }
}