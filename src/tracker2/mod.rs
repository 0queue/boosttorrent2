use boostencode::Value;
use futures::Async;
use futures::Future;
use futures::Stream;
use futures::sync::mpsc;
use futures::try_ready;
use percent_encoding::{percent_encode, QUERY_ENCODE_SET};
use reqwest::async::Client;
use reqwest::async::Decoder;
use reqwest::async::Response;
use reqwest::Url;
use serde::ser::SerializeMap;
use serde::Serialize;
use serde::Serializer;
use std::io::Cursor;
use std::net::SocketAddr;
use tokio::prelude::*;
use std::net::IpAddr;

pub enum Event {
    Started,
    Completed,
    Stopped,
}

impl Serialize for Event {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where
        S: Serializer {
        let s = match self {
            Event::Started => "started",
            Event::Completed => "completed",
            Event::Stopped => "stopped",
        };

        serializer.serialize_str(s)
    }
}

#[derive(Clone)]
pub struct TrackerInfo {
    address: Url,
    info_hash: [u8; 20],
    peer_id: [u8; 20],
    port: u16,
    client: Client,
}

// TODO move somewhere else
pub struct Stats {
    pub uploaded: u64,
    pub downloaded: u64,
    pub left: u64,
}

// could probably derive this
impl Serialize for Stats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where
        S: Serializer {
        let mut mapper = serializer.serialize_map(None)?;
        mapper.serialize_entry("uploaded", &format!("{}", self.uploaded))?;
        mapper.serialize_entry("downloaded", &format!("{}", self.downloaded))?;
        mapper.serialize_entry("left", &format!("{}", self.left))?;
        mapper.end()
    }
}


impl TrackerInfo {
    pub fn new(address: Url, info_hash: [u8; 20], peer_id: [u8; 20], port: u16) -> TrackerInfo {
        TrackerInfo { address, info_hash, peer_id, port, client: Client::new() }
    }

    pub fn send_event(&self, stats: &Stats, event: Event) -> impl Future<Item=Value, Error=()> {
        self.client.get(self.address.clone())
            .query(&self)
            .query(&[("event", event)])
            .query(stats)
            .send()
            .and_then(|mut res| {
                let body = std::mem::replace(res.body_mut(), Decoder::empty());
                body.concat2()
            })
            .map_err(|err| println!("err: {:?}", err))
            .map(|body| {
                let mut body = Cursor::new(body);
                let bytes = body.bytes().collect::<Result<Vec<_>, _>>().unwrap();
                Value::decode(bytes.as_ref()).unwrap()
            })
    }
}

impl Serialize for TrackerInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where
        S: Serializer {
        let mut mapper = serializer.serialize_map(None)?;
        mapper.serialize_entry("info_hash", &percent_encode(&self.info_hash, QUERY_ENCODE_SET).to_string())?;
        mapper.serialize_entry("peer_id", &percent_encode(&self.peer_id, QUERY_ENCODE_SET).to_string())?;
        mapper.serialize_entry("port", &format!("{}", self.port))?;
        mapper.end()
    }
}