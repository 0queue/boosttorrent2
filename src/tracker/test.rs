use boostencode::Value;
use hyper::{
    Body,
    Request,
    Response,
    server::Server,
};
use maplit::hashmap;
use server::SharedState;
use std::net::IpAddr;
use super::*;
use tokio;


fn service_handler(request_: Request<Body>) -> Response<Body> {
    let res = String::from_utf8(Value::Dict(hashmap! {
                    Vec::from("interval") => Value::Integer(10),
                    Vec::from("tracker id") => Value::BString(Vec::from("i am the tracker")),
                    Vec::from("complete") => Value::Integer(10),
                    Vec::from("incomplete") => Value::Integer(10),
                    Vec::from("peers") => Value::List(vec![Value::Dict(hashmap!{
                        Vec::from("peer id") => Value::BString(vec![1;20]),
                        Vec::from("ip") => Value::BString(Vec::from("127.0.0.1")),
                        Vec::from("port") => Value::Integer(8888)
                    })])
                }).encode()).unwrap();

    println!("{}", res);
    Response::builder()
        .status(200)
        .header("Content-Type", "text/plain")
        .body(res.into()).unwrap()
}

#[test]
fn test_announce() {
    let address: (IpAddr, u16) = ([127, 0, 0, 1].into(), 8888);
    let state = SharedState::default();
    let test_server = Server::bind(&address.into())
        .serve(|| {
            hyper::service::service_fn_ok(service_handler)
        }).map_err(|_| ());
    let mut runtime = tokio::runtime::Runtime::new().unwrap();
    let executor = runtime.executor();
    executor.spawn(test_server);

    let mut tracker = Tracker::new(
        "http://localhost:8888".to_owned(),
        [0; 20],
        8888,
        state);
    tracker.start();
    let start_resp = runtime.block_on(tracker).expect("start should not return error");

    assert_eq!(start_resp, TrackerResponse::Success(
        TrackerSuccessResponse {
            interval: 10,
            min_interval: None,
            tracker_id: Some("i am the tracker".to_owned()),
            complete: 10,
            incomplete: 10,
            peers: vec![PeerInfo {
                peer_id: Some([1; 20]),
                address: address.into(),
            }],
        }
    ));

    runtime.shutdown_now();
}