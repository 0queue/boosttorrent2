use crate::boostencode::{DecodeError, FromValue, Value};
use actix::{
    Addr,
    Message,
    Actor,
    Handler,
    ResponseActFuture,
    fut::{
        ActorFuture,
        wrap_future,
    },
};
use hyper;
use hyper::{
    Client,
    http::uri::InvalidUri,
    StatusCode,
};
use log::trace;
use maplit::hashmap;
use percent_encoding::{
    percent_encode,
    QUERY_ENCODE_SET,
};
//use std::fmt;
use std::net::{
    IpAddr,
    SocketAddr,
};
use tokio::prelude::{
    Future,
    future::{
        err,
        ok,
    },
    Stream,
};
use std::time::{
    Instant,
    Duration,
};
use crate::stats::{
    Stats,
    GetStats
};

pub struct Tracker {
    coordinator: Addr<Stats>,
    // The 20 byte unique identifier for this instance of the client
    peer_id: [u8; 20],
    // The uri of the tracker
    tracker_uri: String,
    // The SHA1 hash of the value of the info key in the torrent file
    info_hash: [u8; 20],
    // The port we will be listening on for peer connections
    port: u16,
    // A string the client should send on subsequent announcements
    tracker_id: Option<String>,
    // A cache of the last response
    response_cache: Option<TrackerSuccessResponse>,
    // The time after which we can make another refresh request
    cache_until: Instant,
}

#[derive(Debug, PartialEq, Clone)]
pub struct PeerInfo {
    // the unique identifier for this peer
    pub peer_id: Option<[u8; 20]>,
    // The ip/port of this peer
    pub address: SocketAddr,
}

#[derive(Debug, PartialEq, Clone)]
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

#[derive(Debug, PartialEq, Clone)]
pub enum TrackerResponse {
    Failure(String),
    Warning(String, TrackerSuccessResponse),
    Success(TrackerSuccessResponse),
}


#[derive(Debug, derive_error::Error)]
pub enum TrackerError {
    /// We could not get upload/download statistics to send to the tracker
    CouldNotGetStats,
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

impl FromValue for PeerInfo {
    type Error = String;

    fn from_value(val: &Value) -> Result<Self, Self::Error> {
        let map = val.dict().ok_or("Not a dictionary".to_string())?;

        let mut peer_id: [u8; 20] = [0; 20];
        map.get("peer id".as_bytes()).and_then(Value::bstring)
            .map(|bytes| peer_id.copy_from_slice(bytes))
            .ok_or("Missing key: peer id".to_string())?;
        let peer_id = Some(peer_id);

        let ip = map.get("ip".as_bytes()).and_then(Value::bstring_utf8)
            .map(|s| s.parse())
            .ok_or("Missing key: ip".to_string())?
            .map_err(|_| "Invalid ip addr".to_string())?;

        let port = map.get("port".as_bytes()).and_then(Value::integer)
            .map(|i| *i as u16)
            .ok_or("Missing key: port".to_string())?;

        Ok(PeerInfo {
            peer_id,
            address: SocketAddr::new(ip, port),
        })
    }
}

impl FromValue for TrackerResponse {
    type Error = String;

    fn from_value(val: &Value) -> Result<Self, Self::Error> {
        let map = val.dict().ok_or("Not a dictionary".to_string())?;

        if let Some(msg) = map.get("failure reason".as_bytes()) {
            return Err(msg.bstring_utf8().unwrap_or("unknown failure reason".to_string()));
        };

        let warning_msg = map.get("warning message".as_bytes()).and_then(Value::bstring_utf8);


        let interval = map.get("interval".as_bytes()).and_then(Value::integer)
            .map(|i| *i as u32)
            .ok_or("Missing key: interval".to_string())?;

        let min_interval = map.get("min interval".as_bytes()).and_then(Value::integer)
            .map(|i| *i as u32);

        let tracker_id = map.get("tracker id".as_bytes()).and_then(Value::bstring_utf8);

        let complete = map.get("complete".as_bytes()).and_then(Value::integer)
            .map(|i| *i as u32)
            .ok_or("Missing key: complete".to_string())?;

        let incomplete = map.get("incomplete".as_bytes()).and_then(Value::integer)
            .map(|i| *i as u32)
            .ok_or("Missing key: incomplete".to_string())?;

        let peers = match map.get("peers".as_bytes()).ok_or("Missing key: peers".to_string())? {
            // Dictionary model
            Value::List(peers) => peers.iter()
                .map(PeerInfo::from_value)
                .collect::<Result<Vec<_>, _>>()?,
            // Binary model
            Value::BString(peers) => peers.chunks(6)
                .map(|peer_slice| {
                    // port is in big endian.  multiply instead of bitshift so you can't mess up endianness
                    let port = (peer_slice[0] as u16 * 256) + peer_slice[1] as u16;
                    let mut ip_bytes: [u8; 4] = [0; 4];
                    ip_bytes.copy_from_slice(&peer_slice[..4]);
                    let ip: IpAddr = ip_bytes.into();
                    PeerInfo {
                        peer_id: None,
                        address: (ip, port).into(),
                    }
                })
                .collect(),
            _ => return Err("peers is not in the correct form".to_owned())
        };

        let res = TrackerSuccessResponse {
            interval,
            min_interval,
            tracker_id,
            complete,
            incomplete,
            peers,
        };

        match warning_msg {
            Some(msg) => Ok(TrackerResponse::Warning(msg, res)),
            None => Ok(TrackerResponse::Success(res))
        }
    }
}

impl Tracker {
    /// Create a new Tracker
    pub fn new(
        coordinator: Addr<Stats>,
        peer_id: [u8; 20],
        tracker_uri: String,
        info_hash: [u8; 20],
        port: u16) -> Self {
        Tracker {
            coordinator,
            peer_id,
            tracker_uri,
            info_hash,
            port,
            tracker_id: None,
            response_cache: None,
            cache_until: Instant::now(),
        }
    }
}

impl Actor for Tracker {
    type Context = actix::Context<Self>;
}

pub enum Event {
    Start,
    Refresh,
    Stop,
    Complete,
}

impl Message for Event {
    type Result = Result<TrackerResponse, TrackerError>;
}

impl Handler<Event> for Tracker {
    type Result = ResponseActFuture<Self, TrackerResponse, TrackerError>;

    fn handle(&mut self, msg: Event, _ctx: &mut Self::Context) -> Self::Result {
        // Only use the cached value if this is a refresh request and it is before the cache timeout
        if let (Some(resp), Event::Refresh) = (&self.response_cache, &msg) {
            if Instant::now() < self.cache_until {
                return Box::new(actix::fut::ok(TrackerResponse::Success(resp.clone())));
            }
        }
        // Don't use the cached value, actually reach out to the tracker
        let event = match msg {
            Event::Start => "started",
            Event::Refresh => "",
            Event::Stop => "stopped",
            Event::Complete => "completed",
        };
        let mut req_uri = self.tracker_uri.clone();
        let encoded_info_hash = percent_encode(&self.info_hash, QUERY_ENCODE_SET).to_string();
        let encoded_peer_id = percent_encode(&self.peer_id, QUERY_ENCODE_SET).to_string();
        let query = hashmap! {
            "info_hash" => encoded_info_hash,
            "peer_id" => encoded_peer_id,
            "port" => self.port.to_string(),
            "compact" => 0.to_string(),
        };
        req_uri.push('?');
        query.iter().fold(&mut req_uri, |s, (k, v)| {
            s.push_str(k);
            s.push('=');
            s.push_str(&v);
            s.push('&');
            s
        });
        if event != "" {
            req_uri.push_str("event");
            req_uri.push('=');
            req_uri.push_str(event);
            req_uri.push('&')
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
        let uri = self.coordinator.send(GetStats)
            .map_err(|_| TrackerError::CouldNotGetStats)
            .and_then(move |stats| {
                let query = hashmap! {
                    "uploaded" => stats.uploaded.to_string(),
                    "downloaded" => stats.downloaded.to_string(),
                    "left" => stats.left.to_string()
                };
                query.iter().fold(&mut req_uri, |s, (k, v)| {
                    s.push_str(k);
                    s.push('=');
                    s.push_str(&v);
                    s.push('&');
                    s
                });
                let _ = req_uri.pop();
                match hyper::http::HttpTryFrom::try_from(&req_uri) {
                    Ok(uri) => ok(uri),
                    Err(e) => err(TrackerError::InvalidURI(e))
                }
            });
        // Start the tracker query future
        let client = Client::new();
        let request = uri.and_then(move |uri| {
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
            trace!("response: {:?}", val);
            TrackerResponse::from_value(&val)
                .map_err(|_| TrackerError::InvalidResponse)
        });
        // this wrapper future allows the actor to update its state based on the result of the completed future
        let update_self = wrap_future::<_, Self>(request).map(|result, mut actor, _ctx| {
            match &result {
                TrackerResponse::Warning(_, resp) | TrackerResponse::Success(resp) => {
                    actor.tracker_id = resp.tracker_id.clone();
                    actor.response_cache = Some(resp.clone());
                    actor.cache_until = Instant::now() + Duration::new(resp.interval as u64, 0);
                }
                _ => ()
            };
            result
        });
        Box::new(update_self)
    }
}
