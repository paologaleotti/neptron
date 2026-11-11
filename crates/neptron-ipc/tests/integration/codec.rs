//! The request and response lines: the envelope, the version and every way
//! a line can be wrong.

use neptron_ipc::{
    ClockStepArgs, CodecError, Command, ErrorBody, ErrorCode, FrameReached, MIN_PROTOCOL_VERSION,
    PROTOCOL_VERSION, Refusal, Reply, Request, Response, decode_request, decode_response,
    encode_request, encode_response,
};

use crate::messages::{commands, replies};

#[test]
fn an_unsupported_version_error_line_carries_the_range() {
    let r = refusal(r#"{"v":7,"id":3,"cmd":"hello"}"#);
    let line = encode_response(&Response {
        version: r.version,
        id: r.id,
        outcome: Err(r.error),
    })
    .unwrap();
    assert_eq!(
        String::from_utf8(line).unwrap(),
        concat!(
            r#"{"v":1,"id":3,"error":{"code":"unsupported_version","#,
            r#""message":"version 7 is not served; this server speaks 1 to 1","#,
            r#""min_version":1,"max_version":1}}"#,
            "\n"
        )
    );
}

#[test]
fn a_request_keeps_its_version() {
    let request = decode_request(br#"{"v":1,"id":3,"cmd":"hello"}"#).unwrap();
    assert_eq!(request.version, 1);
    let r = refusal(r#"{"v":1,"id":3,"cmd":"warp"}"#);
    assert_eq!(r.version, 1);
}

#[test]
fn a_key_twice_in_one_object_is_an_invalid_request() {
    for line in [
        r#"{"v":1,"id":3,"id":4,"cmd":"hello"}"#,
        r#"{"v":1,"id":3,"cmd":"hello","cmd":"stats"}"#,
        r#"{"v":1,"id":3,"cmd":"clock_step","args":{"frames":1,"frames":2}}"#,
        r#"{"v":1,"id":3,"cmd":"settings_set","args":{"key":"k","level":"global","value":[{"a":1,"a":2}]}}"#,
    ] {
        let r = refusal(line);
        assert_eq!(r.error.code, ErrorCode::InvalidRequest, "{line}");
        assert_eq!(r.id, None, "{line}");
        assert!(r.error.message.contains("twice"), "{}", r.error.message);
    }
}

#[test]
fn equal_keys_in_different_objects_are_accepted() {
    let line = r#"{"v":1,"id":3,"cmd":"settings_set","args":{"key":"k","level":"global","value":{"key":1}}}"#;
    assert!(decode_request(line.as_bytes()).is_ok());
}

fn refusal(line: &str) -> Refusal {
    decode_request(line.as_bytes()).unwrap_err()
}

#[test]
fn the_protocol_version_is_one() {
    assert_eq!(PROTOCOL_VERSION, 1);
}

#[test]
fn every_request_line_is_pinned_and_round_trips() {
    for (command, json) in commands() {
        let request = Request {
            version: 1,
            id: 42,
            command,
        };
        let line = format!(r#"{{"v":1,"id":42,{}"#, &json[1..]) + "\n";
        assert_eq!(
            String::from_utf8(encode_request(&request).unwrap()).unwrap(),
            line
        );
        assert_eq!(decode_request(line.trim_end().as_bytes()).unwrap(), request);
    }
}

#[test]
fn every_reply_line_is_pinned_and_round_trips() {
    for (reply, json) in replies() {
        let response = Response {
            version: 1,
            id: Some(42),
            outcome: Ok(reply),
        };
        let line = format!(r#"{{"v":1,"id":42,{}"#, &json[1..]) + "\n";
        assert_eq!(
            String::from_utf8(encode_response(&response).unwrap()).unwrap(),
            line
        );
        assert_eq!(
            decode_response(line.trim_end().as_bytes()).unwrap(),
            response
        );
    }
}

#[test]
fn an_error_line_is_pinned_and_round_trips() {
    let response = Response {
        version: 1,
        id: None,
        outcome: Err(ErrorBody::new(ErrorCode::Malformed, "bad")),
    };
    let line = r#"{"v":1,"id":null,"error":{"code":"malformed","message":"bad"}}"#;
    assert_eq!(
        encode_response(&response).unwrap(),
        format!("{line}\n").as_bytes()
    );
    assert_eq!(decode_response(line.as_bytes()).unwrap(), response);
}

#[test]
fn key_order_in_a_request_does_not_matter() {
    let line = r#"{"args":{"frames":3},"cmd":"clock_step","id":9,"v":1}"#;
    assert_eq!(
        decode_request(line.as_bytes()).unwrap(),
        Request {
            version: 1,
            id: 9,
            command: Command::ClockStep(ClockStepArgs { frames: 3 }),
        }
    );
}

#[test]
fn text_that_is_not_json_is_malformed() {
    for line in ["", "   ", "{", "hello", "{\"v\":1,}"] {
        let r = refusal(line);
        assert_eq!(r.id, None, "{line:?}");
        assert_eq!(r.error.code, ErrorCode::Malformed, "{line:?}");
    }
}

#[test]
fn bytes_that_are_not_utf8_are_malformed() {
    let r = decode_request(b"{\"v\":1,\"id\":1,\"cmd\":\"\xff\"}").unwrap_err();
    assert_eq!(r.error.code, ErrorCode::Malformed);
}

#[test]
fn json_that_is_not_an_object_is_an_invalid_request() {
    for line in ["[]", "1", "\"hello\"", "null"] {
        let r = refusal(line);
        assert_eq!(r.error.code, ErrorCode::InvalidRequest, "{line:?}");
        assert_eq!(r.id, None);
    }
}

#[test]
fn a_bad_or_missing_id_is_an_invalid_request_without_an_id() {
    for line in [
        r#"{"v":1,"cmd":"hello"}"#,
        r#"{"v":1,"id":-1,"cmd":"hello"}"#,
        r#"{"v":1,"id":"1","cmd":"hello"}"#,
        r#"{"v":1,"id":1.5,"cmd":"hello"}"#,
    ] {
        let r = refusal(line);
        assert_eq!(r.error.code, ErrorCode::InvalidRequest, "{line}");
        assert_eq!(r.id, None, "{line}");
    }
}

#[test]
fn a_bad_or_missing_version_is_an_invalid_request() {
    for line in [
        r#"{"id":3,"cmd":"hello"}"#,
        r#"{"v":"1","id":3,"cmd":"hello"}"#,
        r#"{"v":4294967296,"id":3,"cmd":"hello"}"#,
    ] {
        let r = refusal(line);
        assert_eq!(r.error.code, ErrorCode::InvalidRequest, "{line}");
        assert_eq!(r.id, Some(3), "{line}");
    }
}

#[test]
fn a_version_above_the_server_or_zero_is_unsupported() {
    for v in [0, PROTOCOL_VERSION + 1] {
        let r = refusal(&format!(r#"{{"v":{v},"id":3,"cmd":"hello"}}"#));
        assert_eq!(r.error.code, ErrorCode::UnsupportedVersion);
        assert_eq!(r.id, Some(3));
        assert_eq!(r.version, PROTOCOL_VERSION);
        assert_eq!(r.error.min_version, Some(MIN_PROTOCOL_VERSION));
        assert_eq!(r.error.max_version, Some(PROTOCOL_VERSION));
        assert!(
            r.error.message.contains(&v.to_string()),
            "{}",
            r.error.message
        );
    }
}

#[test]
fn an_unknown_key_in_the_envelope_is_an_invalid_request() {
    let r = refusal(r#"{"v":1,"id":3,"cmd":"hello","token":"x"}"#);
    assert_eq!(r.error.code, ErrorCode::InvalidRequest);
    assert!(r.error.message.contains("token"), "{}", r.error.message);
}

#[test]
fn a_missing_or_non_string_cmd_is_an_invalid_request() {
    for line in [r#"{"v":1,"id":3}"#, r#"{"v":1,"id":3,"cmd":7}"#] {
        let r = refusal(line);
        assert_eq!(r.error.code, ErrorCode::InvalidRequest, "{line}");
        assert_eq!(r.id, Some(3));
    }
}

#[test]
fn an_unknown_command_is_named_in_the_error() {
    let r = refusal(r#"{"v":1,"id":3,"cmd":"self_destruct"}"#);
    assert_eq!(r.error.code, ErrorCode::UnknownCommand);
    assert_eq!(r.id, Some(3));
    assert!(
        r.error.message.contains("self_destruct"),
        "{}",
        r.error.message
    );
}

#[test]
fn a_command_name_is_case_sensitive() {
    let r = refusal(r#"{"v":1,"id":3,"cmd":"Hello"}"#);
    assert_eq!(r.error.code, ErrorCode::UnknownCommand);
}

#[test]
fn an_unknown_argument_key_is_named_in_the_error() {
    let r = refusal(r#"{"v":1,"id":3,"cmd":"clock_step","args":{"frames":1,"speed":2}}"#);
    assert_eq!(r.error.code, ErrorCode::InvalidArguments);
    assert_eq!(r.id, Some(3));
    assert!(r.error.message.contains("speed"), "{}", r.error.message);
}

#[test]
fn wrong_arguments_are_invalid_arguments() {
    for line in [
        r#"{"v":1,"id":3,"cmd":"clock_step"}"#,
        r#"{"v":1,"id":3,"cmd":"clock_step","args":{}}"#,
        r#"{"v":1,"id":3,"cmd":"clock_step","args":{"frames":-1}}"#,
        r#"{"v":1,"id":3,"cmd":"clock_step","args":{"frames":4294967296}}"#,
        r#"{"v":1,"id":3,"cmd":"screenshot","args":{"stage":"crt"}}"#,
        r#"{"v":1,"id":3,"cmd":"input_send","args":{"port":0,"control":"turbo","value":1}}"#,
        r#"{"v":1,"id":3,"cmd":"input_send","args":{"port":0,"control":"a","value":40000}}"#,
        r#"{"v":1,"id":3,"cmd":"ui_tree","args":{"depth":2}}"#,
    ] {
        let r = refusal(line);
        assert_eq!(r.error.code, ErrorCode::InvalidArguments, "{line}");
        assert_eq!(r.id, Some(3), "{line}");
    }
}

#[test]
fn deep_nesting_is_refused_without_a_crash() {
    let deep = format!(
        r#"{{"v":1,"id":3,"cmd":"settings_set","args":{{"key":"k","level":"global","value":{}{}}}}}"#,
        "[".repeat(10_000),
        "]".repeat(10_000)
    );
    let r = refusal(&deep);
    assert_eq!(r.error.code, ErrorCode::Malformed);
}

#[test]
fn a_response_ignores_unknown_keys() {
    let line = r#"{"v":2,"id":5,"reply":"clock_step","data":{"frame":8},"trace":"x"}"#;
    assert_eq!(
        decode_response(line.as_bytes()).unwrap(),
        Response {
            version: 2,
            id: Some(5),
            outcome: Ok(Reply::ClockStep(FrameReached { frame: 8 })),
        }
    );
}

#[test]
fn a_broken_response_is_a_codec_error() {
    assert!(matches!(decode_response(b"{"), Err(CodecError::Syntax(_))));
    for line in [
        r#"[]"#,
        r#"{"id":1,"reply":"clock_step","data":{"frame":8}}"#,
        r#"{"v":1,"id":"x","reply":"clock_step","data":{"frame":8}}"#,
        r#"{"v":1,"id":1,"reply":"warp","data":{}}"#,
        r#"{"v":1,"id":1}"#,
        r#"{"v":1,"id":1,"error":{"code":7,"message":""}}"#,
        r#"{"v":-1,"id":1,"reply":"clock_step","data":{"frame":8}}"#,
    ] {
        assert!(
            matches!(decode_response(line.as_bytes()), Err(CodecError::Shape(_))),
            "{line}"
        );
    }
}

#[test]
fn an_error_code_of_a_newer_server_arrives_as_an_error() {
    let line = r#"{"v":2,"id":1,"error":{"code":"rate_limited","message":"slow"}}"#;
    let response = decode_response(line.as_bytes()).unwrap();
    assert_eq!(response.version, 2);
    assert_eq!(response.outcome.unwrap_err().code, ErrorCode::Unknown);
}
