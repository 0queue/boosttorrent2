use std::io::Cursor;
use std::time::Duration;

use futures::Future;
use futures::Stream;
use reqwest::{
    async::Client,
    async::Decoder,
    Url,
};
use serde::ser::SerializeMap;
use serde::Serialize;
use serde::Serializer;
use tokio::prelude::*;

use boostencode::FromValue;
use boostencode::Value;
use peer::PeerInfo;

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
pub struct Tracker {
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

#[derive(Debug)]
pub struct Success(Duration, Vec<PeerInfo>);

impl FromValue for Success {
    type Error = String;

    fn from_value(val: &Value) -> Result<Self, Self::Error> where Self: Sized {
        let map = val.dict().ok_or("Not a dictionary".to_string())?;

        if let Some(reason) = map.get("failure reason".as_bytes()) {
            return Err(reason.bstring_utf8().unwrap_or("unknown failure reason".to_string()));
        }

        let interval = map.get("interval".as_bytes()).and_then(Value::integer)
            .map(|i| Duration::from_secs(*i as u64))
            .ok_or("Missing key: interval".to_string())?;

        let peers = match map.get("peers".as_bytes()).ok_or("Missing key: peers".to_string())? {
            Value::List(dict) => dict.iter().map(PeerInfo::from_value).collect::<Result<Vec<_>, _>>(),
            Value::BString(bytes) => Ok(PeerInfo::from_compact(bytes)),
            _ => return Err("Invalid peers format".to_string()),
        }?;

        Ok(Success(interval, peers))
    }
}


impl Tracker {
    pub fn new(address: Url, info_hash: [u8; 20], peer_id: [u8; 20], port: u16) -> Tracker {
        Tracker { address, info_hash, peer_id, port, client: Client::new() }
    }

    pub fn send_event(&self, stats: &Stats, event: Event) -> impl Future<Item=Success, Error=String> {
        self.client.get(self.address.clone())
            .query(&self)
            .query(&[("event", event)])
            .query(&[("compact", "1")])
            .query(stats)
            .send()
            .and_then(|mut res| {
                // copy out the body_mut essentially, to avoid lifetime issues
                std::mem::replace(res.body_mut(), Decoder::empty()).concat2()
            })
            .map_err(|err| format!("{}", err))
            .and_then(|body| {
                let body = Cursor::new(body);
                let bytes = body.bytes().collect::<Result<Vec<_>, _>>().unwrap();

                futures::done(Value::decode(&bytes).map(|val| Success::from_value(&val))
                    .map_err(|err| err.to_string())
                    .and_then(|r| r))
            })
    }
}

impl Serialize for Tracker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where
        S: Serializer {
        let mut mapper = serializer.serialize_map(None)?;
        // explanation: percent_encoding then giving to reqwest re-encodes it, so % turns into %25
        // it can't seem to serialize a plain old array of bytes, so force it into a string and pass
        // it on.  Not really an issue
        let raw_info_hash = unsafe { String::from_utf8_unchecked(self.info_hash.to_vec()) };
        mapper.serialize_entry("info_hash", &raw_info_hash)?;
        mapper.serialize_entry("peer_id", &String::from_utf8(self.peer_id.to_vec()).unwrap())?;
        mapper.serialize_entry("port", &format!("{}", self.port))?;
        mapper.end()
    }
}