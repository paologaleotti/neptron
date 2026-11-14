//! The server loop over byte streams, with no socket.

use std::io::{self, BufRead, Read, Write};

use neptron_ipc::{
    Command, ErrorCode, MAX_REQUEST_LINE_BYTES, Reply, Request, Response, ServeError,
    decode_response, dispatch, encode_request, serve_lines,
};

use crate::fixture::{EmptyHandler, FixedHandler};
use crate::messages::commands;

fn serve(input: &[u8], handler: &mut FixedHandler) -> Vec<Response> {
    let mut reader = input;
    let mut out = Vec::new();
    serve_lines(&mut reader, &mut out, handler).unwrap();
    out.split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .map(|l| decode_response(l).unwrap())
        .collect()
}

fn request_line(id: u64, command: Command) -> Vec<u8> {
    encode_request(&Request {
        version: 1,
        id,
        command,
    })
    .unwrap()
}

#[test]
fn every_command_gets_the_reply_of_the_same_name() {
    for (command, _) in commands() {
        let name = command.name();
        let reply = dispatch(&mut FixedHandler::default(), command).unwrap();
        assert_eq!(reply.name(), name);
    }
}

#[test]
fn hello_names_the_protocol_and_the_server() {
    let reply = dispatch(&mut EmptyHandler, Command::Hello).unwrap();
    let Reply::Hello(hello) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(hello.protocol, 2);
    assert_eq!(hello.server, "empty");
}

#[test]
fn a_command_the_handler_does_not_serve_is_unavailable() {
    for (command, _) in commands().into_iter().skip(1) {
        let name = command.name();
        let err = dispatch(&mut EmptyHandler, command).unwrap_err();
        assert_eq!(err.code, ErrorCode::Unavailable, "{name}");
        assert!(err.message.contains(name), "{}", err.message);
    }
}

#[test]
fn one_response_per_line_in_order_with_the_ids() {
    let mut input = Vec::new();
    for (id, (command, _)) in commands().into_iter().enumerate() {
        input.extend(request_line(id as u64, command));
    }
    let responses = serve(&input, &mut FixedHandler::default());
    assert_eq!(responses.len(), Command::NAMES.len());
    for (id, (response, name)) in responses.iter().zip(Command::NAMES).enumerate() {
        assert_eq!(response.id, Some(id as u64));
        assert_eq!(response.outcome.as_ref().unwrap().name(), *name);
    }
}

#[test]
fn the_handler_keeps_its_state_across_lines() {
    let input = [
        r#"{"v":1,"id":1,"cmd":"clock_step","args":{"frames":2}}"#,
        r#"{"v":1,"id":2,"cmd":"clock_step","args":{"frames":3}}"#,
        "",
    ]
    .join("\n");
    let mut handler = FixedHandler::default();
    let responses = serve(input.as_bytes(), &mut handler);
    assert_eq!(handler.frame, 5);
    assert_eq!(responses.len(), 2);
}

#[test]
fn a_handler_error_is_sent_with_the_request_id() {
    let input = r#"{"v":1,"id":8,"cmd":"wait","args":{"until":{"frame":9},"timeout_ms":10}}"#;
    let responses = serve(input.as_bytes(), &mut FixedHandler::default());
    assert_eq!(responses[0].id, Some(8));
    assert_eq!(
        responses[0].outcome.as_ref().unwrap_err().code,
        ErrorCode::Timeout
    );
}

#[test]
fn bad_lines_get_errors_and_the_connection_goes_on() {
    let input = [
        "not json",
        "",
        r#"{"v":1,"id":2,"cmd":"warp"}"#,
        r#"{"v":1,"id":3,"cmd":"clock_step","args":{"frames":"many"}}"#,
        r#"{"v":9,"id":4,"cmd":"hello"}"#,
        r#"{"v":1,"id":5,"cmd":"hello"}"#,
        "",
    ]
    .join("\n");
    let responses = serve(input.as_bytes(), &mut FixedHandler::default());
    let codes: Vec<Option<ErrorCode>> = responses
        .iter()
        .map(|r| r.outcome.as_ref().err().map(|e| e.code))
        .collect();
    assert_eq!(
        codes,
        [
            Some(ErrorCode::Malformed),
            Some(ErrorCode::Malformed),
            Some(ErrorCode::UnknownCommand),
            Some(ErrorCode::InvalidArguments),
            Some(ErrorCode::UnsupportedVersion),
            None,
        ]
    );
    let ids: Vec<Option<u64>> = responses.iter().map(|r| r.id).collect();
    assert_eq!(ids, [None, None, Some(2), Some(3), Some(4), Some(5)]);
}

#[test]
fn a_last_line_without_a_newline_is_served() {
    let responses = serve(
        br#"{"v":1,"id":1,"cmd":"hello"}"#,
        &mut FixedHandler::default(),
    );
    assert_eq!(responses.len(), 1);
    assert!(responses[0].outcome.is_ok());
}

#[test]
fn a_crlf_line_is_served() {
    let responses = serve(
        b"{\"v\":1,\"id\":1,\"cmd\":\"hello\"}\r\n",
        &mut FixedHandler::default(),
    );
    assert!(responses[0].outcome.is_ok());
}

#[test]
fn a_too_long_line_gets_one_error_and_ends_the_connection() {
    let mut input = request_line(1, Command::Hello);
    input.extend(vec![b' '; MAX_REQUEST_LINE_BYTES + 1]);
    input.push(b'\n');
    input.extend(request_line(3, Command::Hello));
    let responses = serve(&input, &mut FixedHandler::default());
    assert_eq!(responses.len(), 2);
    assert!(responses[0].outcome.is_ok());
    assert_eq!(responses[1].id, None);
    assert_eq!(
        responses[1].outcome.as_ref().unwrap_err().code,
        ErrorCode::LineTooLong
    );
}

#[test]
fn a_line_of_exactly_the_limit_is_served() {
    let mut line = br#"{"v":1,"id":1,"cmd":"hello"}"#.to_vec();
    line.resize(MAX_REQUEST_LINE_BYTES, b' ');
    line.push(b'\n');
    let responses = serve(&line, &mut FixedHandler::default());
    assert!(responses[0].outcome.is_ok());
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("cable cut"))
    }
}

impl BufRead for FailingReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Err(io::Error::other("cable cut"))
    }
    fn consume(&mut self, _: usize) {}
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("peer gone"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_read_failure_ends_the_connection_with_an_error() {
    let err = serve_lines(&mut FailingReader, &mut Vec::new(), &mut EmptyHandler).unwrap_err();
    assert!(matches!(err, ServeError::Read(_)), "{err}");
}

#[test]
fn a_write_failure_ends_the_connection_with_an_error() {
    let mut reader: &[u8] = b"{\"v\":1,\"id\":1,\"cmd\":\"hello\"}\n";
    let err = serve_lines(&mut reader, &mut FailingWriter, &mut EmptyHandler).unwrap_err();
    assert!(matches!(err, ServeError::Write(_)), "{err}");
}

#[test]
fn a_hello_of_a_newer_client_learns_the_versions_the_server_speaks() {
    let responses = serve(
        br#"{"v":3,"id":1,"cmd":"hello"}"#,
        &mut FixedHandler::default(),
    );
    assert_eq!(responses[0].version, 2);
    let error = responses[0].outcome.as_ref().unwrap_err();
    assert_eq!(error.code, ErrorCode::UnsupportedVersion);
    assert_eq!((error.min_version, error.max_version), (Some(1), Some(2)));
}

#[test]
fn a_response_echoes_the_version_of_its_request() {
    let responses = serve(
        b"{\"v\":1,\"id\":1,\"cmd\":\"hello\"}\n{\"v\":1,\"id\":2,\"cmd\":\"warp\"}\n",
        &mut FixedHandler::default(),
    );
    assert!(responses.iter().all(|r| r.version == 1));
}

fn stats_at(version: u32) -> Response {
    let line = encode_request(&Request {
        version,
        id: 9,
        command: Command::Stats,
    })
    .unwrap();
    let mut input = line;
    input.push(b'\n');
    serve(&input, &mut FixedHandler::default()).remove(0)
}

/// `fields_missed` came with version 2: a request of version 1 gets the
/// stats reply with no such key, as that version defines it.
#[test]
fn a_field_of_a_later_version_is_not_sent_to_an_older_request() {
    let v1 = stats_at(1);
    assert_eq!(v1.version, 1);
    let Ok(Reply::Stats(stats)) = v1.outcome else {
        panic!("{v1:?}");
    };
    assert_eq!(stats.fields_missed, None);
    let v2 = stats_at(2);
    let Ok(Reply::Stats(stats)) = v2.outcome else {
        panic!("{v2:?}");
    };
    assert_eq!(stats.fields_missed, Some(3));
}

/// A stats reply of a version 1 server, with no `fields_missed` key, still
/// reads.
#[test]
fn an_older_stats_reply_reads() {
    let line = concat!(
        r#"{"v":1,"id":2,"reply":"stats","data":{"frames_presented":1,"frames_dropped":0,"#,
        r#""frames_repeated":0,"audio_underruns":0,"audio_queued_frames":0,"input_to_frame_ns":0}}"#
    );
    let response = decode_response(line.as_bytes()).unwrap();
    let Ok(Reply::Stats(stats)) = response.outcome else {
        panic!("{response:?}");
    };
    assert_eq!(stats.fields_missed, None);
}
