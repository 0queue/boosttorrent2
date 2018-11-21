use crossbeam_channel::{Sender, SendError};
use futures::{Async, AsyncSink, Sink};

pub struct ChannelSink<T> {
    tx: Option<Sender<T>>,
}

impl<T> ChannelSink<T> {
    pub fn new(chan: crossbeam_channel::Sender<T>) -> ChannelSink<T> {
        ChannelSink { tx: Some(chan) }
    }
}

impl<T> Sink for ChannelSink<T> {
    type SinkItem = T;
    type SinkError = SendError<T>;

    fn start_send(&mut self, item: Self::SinkItem) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        match self.tx {
            Some(ref chan) => {
                chan.send(item)?;
                Ok(AsyncSink::Ready)
            }
            None => Err(SendError(item))
        }
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Result<Async<()>, Self::SinkError> {
        // drop the tx
        self.tx.take();
        Ok(Async::Ready(()))
    }
}