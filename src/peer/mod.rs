use piece::Piece;
use futures::sync::mpsc::{
    Receiver,
    Sender,
};
use tokio::{
    net::TcpStream,
    prelude::{
        Async,
        Future,
        Stream,
        Sink,
        AsyncSink,
    },
    codec::Framed,
};
use bit_vec::BitVec;
use log::error;

mod message;

/// A connection to a peer.  Can download pieces from this connection
pub struct Peer {
    conn: Framed<TcpStream, message::MessageCodec>,
    uploaded_sender: Sender<u32>,
    downloaded_sender: Sender<u32>,
    // When a piece is done, the peer will send the piece to the receiver, along with what pieces
    // this peer has, and a way to send a new piece back
    finished_piece_sender: Sender<(Piece, Sender<Piece>, BitVec)>,
    peers_pieces: BitVec,
    info_hash: [u8; 20],
    peer_id: [u8; 20],
    initiates: bool,
}

impl Peer {
    pub fn new(conn: TcpStream,
               uploaded_sender: Sender<u32>,
               downloaded_sender: Sender<u32>,
               finished_piece_sender: Sender<(Piece, Sender<Piece>, BitVec)>,
               info_hash: [u8; 20],
               peer_id: [u8; 20],
               initiates: bool) -> Self {
        let mut conn = Framed::new(conn, message::MessageCodec::new());
        if initiates {
            let _res = conn.start_send(message::Message::Handshake((info_hash.clone(), peer_id.clone()).into()));
        }
        Peer {
            conn,
            uploaded_sender,
            downloaded_sender,
            finished_piece_sender,
            peers_pieces: BitVec::new(),
            info_hash,
            peer_id,
            initiates,
        }
    }
}

// Peer can be spun into tasks
impl Future for Peer {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        loop {
            match self.conn.poll() {
                Ok(Async::NotReady) => break, // No more messages right now
                Ok(Async::Ready(None)) => return Ok(Async::Ready(())), // connection closed, end the task
                Ok(Async::Ready(Some(message))) => {
                    match message {
                        message::Message::Handshake(item) => {
                            if self.info_hash != item.info_hash {
                                error!("The info hash sent by a peer does not match ours");
                                return Err(())
                            }
                            if !self.initiates {
                                let _res = self.conn.start_send(
                                    message::Message::Handshake(
                                        (self.info_hash.clone(), self.peer_id.clone()).into()
                                    ));
                            }
                        }
                        // TODO Process Message
                        _ => {}
                    }
                }
                Err(e) => {
                    error!("Connection to peer closed with error '{}'", e);
                    return Err(());
                }
            }
        };
        // TODO maybe send a message using self.conn.start_send here
        loop {
            match self.conn.poll_complete() {
                Ok(Async::NotReady) => break, // Can't make anymore progress
                Ok(Async::Ready(())) => (), // All sends complete
                Err(e) => {
                    error!("Connection to peer closed with error '{}'", e);
                    return Err(());
                }
            }
        }
        Ok(Async::NotReady)
    }
}