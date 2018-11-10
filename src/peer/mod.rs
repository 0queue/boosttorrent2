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
    },
};
use bit_vec::BitVec;


/// A connection to a peer.  Can download pieces from this connection
pub struct Peer {
    conn: TcpStream,
    uploaded_sender: Sender<u32>,
    downloaded_sender: Sender<u32>,
    // When a piece is done, the peer will send the piece to the receiver, along with what pieces
    // this peer has, and a way to send a new piece back
    finished_piece_sender: Sender<(Piece, Sender<Piece>, BitVec)>,
    peers_pieces: BitVec
}

impl Peer {
    pub fn new(conn: TcpStream,
               uploaded_sender: Sender<u32>,
               downloaded_sender: Sender<u32>,
               finished_piece_sender: Sender<(Piece, Sender<Piece>, BitVec)>) -> Self {
        Peer {
            conn,
            uploaded_sender,
            downloaded_sender,
            finished_piece_sender,
            peers_pieces: BitVec::new()
        }
    }
}

// Peer can be spun into tasks
impl Future for Peer {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        Ok(Async::Ready(()))
    }
}