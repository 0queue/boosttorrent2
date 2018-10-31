use boostencode::{DecodeError, Value};
use hyper;
use hyper::{
    Client,
    http::uri::InvalidUri,
    StatusCode,
};
use maplit::hashmap;
use percent_encoding::{
    percent_encode,
    QUERY_ENCODE_SET,
};
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::prelude::*;

#[cfg(test)]
mod test;

#[derive(Debug)]
pub struct Tracker {
    // The uri of the tracker
    tracker_uri: String,
    // The SHA1 hash of the value of the info key in the torrent file
    info_hash: [u8; 20],
    // A unique identifier for this instance of the client.  Can be any 20 byte string
    peer_id: [u8; 20],
    // The port we will be listening on for peer connections
    port: u16,
    // The total number of bytes uploaded to peers
    uploaded: Arc<AtomicUsize>,
    // The total number of bytes downloaded from peers
    downloaded: Arc<AtomicUsize>,
    // The number of bytes remaining in the download
    left: Arc<AtomicUsize>,
    // A string the client should send on subsequent announcements
    tracker_id: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct PeerInfo {
    // the unique identifier for this peer
    pub peer_id: [u8; 20],
    // The ip/port of this peer
    pub address: SocketAddr,
}

#[derive(Debug, PartialEq)]
pub struct TrackerSuccessResponse {
    // The number of seconds the client should wait before sending a regular request to the tracker
    pub interval: u32,
    // If present, clients must not re-announce more frequently than this
    pub min_interval: Option<u32>,
    // A string the client should send on subsequent announcements
    pub tracker_id: Option<String>,
    // Number of seeders
    pub complete: u32,
    // Number of leechers,
    pub incomplete: u32,
    // A list of peers that we could connect to
    pub peers: Vec<PeerInfo>,
}

#[derive(Debug, PartialEq)]
pub enum TrackerResponse {
    Failure(String),
    Warning(String, TrackerSuccessResponse),
    Success(TrackerSuccessResponse),
}


#[derive(Debug, derive_error::Error)]
pub enum TrackerError {
    /// The uri we wanted to request was somehow invalid
    InvalidURI(InvalidUri),
    /// Could not connect to the tracker
    ConnectionError(hyper::Error),
    /// The tracker returned a non 200 status code
    #[error(non_std, no_from)]
    ResponseError(u16),
    /// The response body could not be bdecoded
    DecodeError(DecodeError),
    /// The contents of the response are not correct
    InvalidResponse,
}

enum Event {
    Started,
    Stopped,
    Completed,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            Event::Started => "started",
            Event::Stopped => "stopped",
            Event::Completed => "completed"
        })
    }
}

impl PeerInfo {
    fn from_value(val: Value) -> Option<Self> {
        let mut map = match val {
            Value::Dict(m) => m,
            _ => return None
        };
        let mut peer_id: [u8; 20] = [0; 20];
        let peer_id_val = map.remove("peer id".as_bytes())?;
        match peer_id_val {
            Value::BString(s) => peer_id.copy_from_slice(&s[..20]),
            _ => return None
        };
        let ip_val = map.remove("ip".as_bytes())?;
        let port_val = map.remove("port".as_bytes())?;
        let address: SocketAddr = match (ip_val, port_val) {
            (Value::BString(ip), Value::Integer(port)) => {
                let ip_string = String::from_utf8(ip).ok()?;
                let ip_addr = IpAddr::from_str(&ip_string).ok()?;
                (ip_addr, port as u16).into()
            }
            _ => return None,
        };
        Some(PeerInfo {
            peer_id,
            address,
        })
    }
}

impl TrackerResponse {
    fn from_value(val: Value) -> Option<Self> {

        let mut map = match val {
            Value::Dict(m) => m,
            _ => return None
        };

        if let Some(msg) = map.remove("failure reason".as_bytes()) {
            return match msg {
                Value::BString(m) => String::from_utf8(m).ok().map(|s| TrackerResponse::Failure(s)),
                _ => None
            };
        };

        let warning_msg = map.remove("warning message".as_bytes())
            .and_then(|msg| {
                match msg {
                    Value::BString(m) => String::from_utf8(m).ok(),
                    _ => None
                }
            });

        let interval = match map.remove("interval".as_bytes())? {
            Value::Integer(i) => i as u32,
            _ => return None
        };

        let min_interval = map.remove("min interval".as_bytes())
            .and_then(|msg| {
                match msg {
                    Value::Integer(i) => Some(i as u32),
                    _ => None
                }
            });

        let tracker_id = map.remove("tracker id".as_bytes())
            .and_then(|msg| {
                match msg {
                    Value::BString(m) => String::from_utf8(m).ok(),
                    _ => None
                }
            });

        let complete = match map.remove("complete".as_bytes())? {
            Value::Integer(i) => i as u32,
            _ => return None
        };

        let incomplete = match map.remove("incomplete".as_bytes())? {
            Value::Integer(i) => i as u32,
            _ => return None
        };

        let peers = match map.remove("peers".as_bytes())? {
            Value::List(mut list) => list.drain(..)
                .map(PeerInfo::from_value)
                .collect::<Option<Vec<PeerInfo>>>(),
            _ => return None
        }?;

        let res = TrackerSuccessResponse {
            interval,
            min_interval,
            tracker_id,
            complete,
            incomplete,
            peers,
        };

        match warning_msg {
            Some(msg) => Some(TrackerResponse::Warning(msg, res)),
            None => Some(TrackerResponse::Success(res))
        }
    }
}

impl Tracker {
    /// Create a new Tracker
    pub fn new(
        tracker_uri: String,
        info_hash: [u8; 20],
        peer_id: [u8; 20],
        port: u16,
        uploaded: Arc<AtomicUsize>,
        downloaded: Arc<AtomicUsize>,
        left: Arc<AtomicUsize>) -> Self {
        Tracker {
            tracker_uri,
            info_hash,
            peer_id,
            port,
            uploaded,
            downloaded,
            left,
            tracker_id: None,
        }
    }

    /// Tell the tracker that you are starting your download
    pub fn start(&mut self) -> impl future::Future<Item=TrackerResponse, Error=TrackerError> {
        self.announce(Some(Event::Started))
    }

    /// Update the tracker id. This must be set before calling cancel, finish, or refresh
    pub fn update_tracker_id(&mut self, new_id: &str) {
        self.tracker_id = Some(new_id.to_owned());
    }

    /// Tell the tracker that you are stopping your download without finishing.
    pub fn cancel(&self) -> impl future::Future<Item=TrackerResponse, Error=TrackerError> {
        self.announce(Some(Event::Stopped))
    }

    /// Tell the tracker that you have completed the download
    pub fn finish(&self) -> impl future::Future<Item=TrackerResponse, Error=TrackerError> {
        self.announce(Some(Event::Completed))
    }

    /// Update the tracker on your download status, and get more peers
    pub fn refresh(&self) -> impl future::Future<Item=TrackerResponse, Error=TrackerError> {
        self.announce(None)
    }

    fn announce(&self, event: Option<Event>) -> impl future::Future<Item=TrackerResponse, Error=TrackerError> {
        // build the tracker query string
        let mut req_uri = self.tracker_uri.clone();
        let encoded_info_hash = percent_encode(&self.info_hash, QUERY_ENCODE_SET).to_string();
        let encoded_peer_id = percent_encode(&self.peer_id, QUERY_ENCODE_SET).to_string();
        let query = hashmap! {
            "info_hash" => encoded_info_hash,
            "peer_id" => encoded_peer_id,
            "port" => self.port.to_string(),
            "uploaded" => (*self.uploaded).load(Ordering::Relaxed).to_string(),
            "downloaded" => (*self.downloaded).load(Ordering::Relaxed).to_string(),
            "left" => (*self.left).load(Ordering::Relaxed).to_string(),
            "compact" => 0.to_string(),
        };
        req_uri.push('?');
        query.iter().fold(&mut req_uri, | s, (k, v)| {
            s.push_str(k);
            s.push('=');
            s.push_str(&v);
            s.push('&');
            s
        });
        match event {
            Some(e) => {
                req_uri.push_str("event");
                req_uri.push('=');
                req_uri.push_str(&e.to_string());
                req_uri.push('&')
            }
            None => ()
        }
        match &self.tracker_id {
            Some(id) => {
                req_uri.push_str("trackerid");
                req_uri.push('=');
                req_uri.push_str(&percent_encode(id.as_bytes(), QUERY_ENCODE_SET).to_string());
                req_uri.push('&')
            }
            None => ()
        }
        let _ = req_uri.pop();
        let client = Client::new();
        let uri = match hyper::http::HttpTryFrom::try_from(&req_uri) {
            Ok(uri) => future::ok(uri),
            Err(e) => future::err(TrackerError::InvalidURI(e))
        };
        // Start the tracker query future
        uri.and_then(move |uri| {
            client.get(uri).map_err(|e| TrackerError::ConnectionError(e))
        }).and_then(|get_response| {
            if get_response.status() == StatusCode::OK {
                Ok(get_response.into_body())
            } else {
                Err(TrackerError::ResponseError(get_response.status().as_u16()))
            }
        }).and_then(|body| {
            body.map(|chunk| {
                Vec::from(&*chunk)
            }).concat2()
                .map_err(|e| TrackerError::ConnectionError(e))
        }).and_then(|resp_bytes| {
            Value::decode(&resp_bytes).map_err(|e| TrackerError::DecodeError(e))
        }).and_then(|val| {
            TrackerResponse::from_value(val)
                .ok_or(TrackerError::InvalidResponse)
        })
    }
}