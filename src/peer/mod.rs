use piece::Piece;
use futures::sync::mpsc::{
    UnboundedReceiver,
    UnboundedSender,
    SendError,
    unbounded,
};
use tokio::{
    net::TcpStream,
    prelude::{
        Async,
        Future,
        Stream,
        Sink,
        AsyncSink,
        future::{
            Either,
            ok
        },
    },
    codec::Framed,
    spawn,
};
use bit_vec::BitVec;
use log::error;

mod message;

type Rx<T> = UnboundedReceiver<T>;
type Tx<T> = UnboundedSender<T>;

enum Command {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Piece(Piece),
    Have(u32),
    Cancel,
    Disconnect,
}

/// presents a simple interface for interacting with a peer task
pub struct Peer {
    downloaded_stream: Rx<u32>,
    uploaded_stream: Rx<u32>,
    have_stream: Rx<u32>,
    have_map: BitVec,
    piece_stream: Rx<Piece>,
    command_sink: Tx<Command>,
}

/// An asynchronous task to interact with a peer
struct PeerTask {
    conn: Framed<TcpStream, message::MessageCodec>,
    info_hash: [u8; 20],
    download_sink: Tx<u32>,
    upload_sink: Tx<u32>,
    piece_sink: Tx<Piece>,
    have_sink: Tx<u32>,
    command_stream: Rx<Command>,
}

impl PeerTask {
    fn new(conn: Framed<TcpStream, message::MessageCodec>,
           info_hash: [u8; 20],
           download_sink: Tx<u32>,
           upload_sink: Tx<u32>,
           piece_sink: Tx<Piece>,
           have_sink: Tx<u32>,
           command_stream: Rx<Command>) -> Self {
        PeerTask {
            conn,
            info_hash,
            download_sink,
            upload_sink,
            piece_sink,
            have_sink,
            command_stream
        }
    }
}

impl Peer {
    pub fn new(conn: TcpStream, info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        let (downloaded_sink, downloaded_stream) = unbounded();
        let (uploaded_sink, uploaded_stream) = unbounded();
        let (piece_sink, piece_stream) = unbounded();
        let (command_sink, command_stream) = unbounded();
        let (have_sink, have_stream) = unbounded();
        let mut conn = Framed::new(conn, message::MessageCodec::new());
        conn.start_send(message::Message::Handshake((info_hash.clone(), peer_id).into()));
        spawn(PeerTask::new(conn,
                            info_hash,
                            downloaded_sink,
                            uploaded_sink,
                            piece_sink,
                            have_sink,
                            command_stream));
        Peer {
            downloaded_stream,
            uploaded_stream,
            have_stream,
            have_map: BitVec::new(),
            piece_stream,
            command_sink,
        }
    }

    pub fn choke(&self) -> Result<(), SendError<Command>> {
        self.command_sink.unbounded_send(Command::Choke)
    }

    pub fn unchoke(&self) -> Result<(), SendError<Command>> {
        self.command_sink.unbounded_send(Command::Unchoke)
    }

    pub fn interested(&self) -> Result<(), SendError<Command>> {
        self.command_sink.unbounded_send(Command::Interested)
    }

    pub fn uninterested(&self) -> Result<(), SendError<Command>> {
        self.command_sink.unbounded_send(Command::NotInterested)
    }

    pub fn cancel_piece(&self) -> Result<(), SendError<Command>> {
        self.command_sink.unbounded_send(Command::Cancel)
    }

    pub fn start_new_piece(&self, piece: Piece) -> Result<(), SendError<Command>> {
        self.command_sink.unbounded_send(Command::Piece(piece))
    }

    pub fn get_have(&mut self) -> &BitVec {
        // update the bitvec
        loop {
            match self.have_stream.poll() {
                Ok(Async::Ready(Some(idx))) => {
                    self.have_map.set(idx as usize, true);
                },
                _ => break
            }
        };
        &self.have_map
    }

    pub fn have_new_piece(&mut self, piece_idx: u32) -> Result<(), SendError<Command>> {
        self.command_sink.unbounded_send(Command::Have(piece_idx))
    }

    pub fn poll_uploaded(&mut self) -> Option<u32> {
        self.uploaded_stream.poll().ok()
            .and_then(|uploaded| match uploaded {
                Async::NotReady => Some(0),
                Async::Ready(Some(b)) => Some(b),
                Async::Ready(None) => None
            })
    }

    pub fn poll_downloaded(&mut self) -> Option<u32> {
        self.downloaded_stream.poll().ok()
            .and_then(|downloaded| match downloaded {
                Async::NotReady => Some(0),
                Async::Ready(Some(b)) => Some(b),
                Async::Ready(None) => None
            })
    }

    pub fn poll_piece(&mut self) -> Option<Async<Piece>> {
        self.piece_stream.poll().ok()
            .and_then(|piece| match piece {
                Async::Ready(None) => None,
                Async::Ready(Some(x)) => Some(Async::Ready(x)),
                Async::NotReady => Some(Async::NotReady)
            })
    }
}

impl Future for PeerTask {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        // Poll for incoming messages until there are no more
        loop {
            match self.conn.poll() {
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(None)) => return Ok(Async::Ready(())),
                Err(e) => {
                    error!("Task encountered an error polling for messages from peer: {}", e);
                    return Err(())
                },
                Ok(Async::Ready(Some(msg))) => {
                    match msg {
                        message::Message::Handshake(handshake) => {
                            if handshake.info_hash == self.info_hash {
                                error!("Peer handshake failed: Wrong info hash.");
                                return Err(())
                            }
                        },
                        // TODO do stuff for other messages
                        _ => {}
                    }
                }
            }
        };
        // Poll for commands from the server
        loop {
            match self.command_stream.poll() {
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(None)) => return Ok(Async::Ready(())),
                Err(e) => {
                    error!("Task encountered an error polling for messages from server");
                    return Err(())
                },
                Ok(Async::Ready(Some(cmd))) => {
                    match cmd {
                        _ => () // TODO Handle commands from the server
                    }
                }
            }
        }
        // TODO figure out a message to send to the peer and call start_send
        // poll the completion of the send
        if let Err(e) = self.conn.poll_complete() {
            error!("An error occured sending a message to the peer: {}", e);
            return Err(())
        }
        Ok(Async::NotReady)
    }
}
