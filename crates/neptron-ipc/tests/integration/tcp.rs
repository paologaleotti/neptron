//! The TCP listener on the loopback address. It exists only with the
//! `control` feature.

use std::net::{Ipv4Addr, SocketAddr};
use std::thread;
use std::time::Duration;

use neptron_ipc::{
    Client, ClockStepArgs, Command, FrameReached, Reply, TcpControlListener, serve_stream,
};

use crate::fixture::FixedHandler;

#[test]
fn a_client_calls_a_server_over_tcp() {
    let listener = TcpControlListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let stream = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut handler = FixedHandler::default();
        serve_stream(&stream, &mut handler).unwrap();
        handler
    });
    let mut client = Client::connect_tcp(addr).unwrap();
    client.set_timeout(Some(Duration::from_secs(10))).unwrap();
    assert_eq!(
        client
            .call(Command::ClockStep(ClockStepArgs { frames: 2 }))
            .unwrap(),
        Reply::ClockStep(FrameReached { frame: 2 })
    );
    drop(client);
    assert_eq!(server.join().unwrap().frame, 2);
}
