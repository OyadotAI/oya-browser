/** Framing tests include incomplete and hostile input without external processes. */
use mars::constants::{FRAME_BYTES, ID_BYTES};
use mars::protocol::{Request, decode, encode, read_frame};
use serde_json::{Value, json};
use std::io::{BufReader, Cursor};

/** A missing newline cannot grow the read buffer beyond its configured frame bound. */
#[test]
fn oversized_unterminated_frames_are_rejected_during_reading() {
    let input = vec![b'x'; FRAME_BYTES + 1];
    let mut reader = BufReader::with_capacity(7, Cursor::new(input));
    assert_eq!(read_frame(&mut reader).unwrap_err().code, "limit_exceeded");
}

/** EOF with partial bytes is an error, while empty EOF closes the session. */
#[test]
fn incomplete_frames_never_become_requests() {
    assert!(read_frame(&mut Cursor::new(b"")).unwrap().is_none());
    assert_eq!(
        read_frame(&mut Cursor::new(b"{}")).unwrap_err().code,
        "invalid_request"
    );
}

/** Newlines and terminal escape bytes inside page text remain data on the wire. */
#[test]
fn machine_output_escapes_page_controlled_text() {
    let value = json!({"text":"first\n\u{1b}[31msecond"});
    let bytes = encode(&value).unwrap();
    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert!(!bytes.contains(&0x1b));
    assert_eq!(decode::<Value>(&bytes).unwrap(), value);
}

/** Protocol versions, IDs and unknown fields are checked before dispatch. */
#[test]
fn request_envelopes_are_strict() {
    let valid = json!({"version":2,"id":"x","op":"page.inspect","params":{}});
    let mut bad = valid.clone();
    bad["extra"] = json!(true);
    assert!(decode::<Request>(&encode(&bad).unwrap()).is_err());
    let mut request: Request = decode(&encode(&valid).unwrap()).unwrap();
    request.version = 1;
    assert!(
        request
            .validate()
            .unwrap_err()
            .message
            .contains(&format!("protocol version {}", mars::constants::VERSION))
    );
    request.id = "x".repeat(ID_BYTES + 1);
    assert!(request.validate().is_err());
    request.id = "\n".into();
    assert!(request.validate().is_err());
}

/** Output includes JSON expansion in its budget, not just original string length. */
#[test]
fn serialized_output_has_a_bound() {
    assert_eq!(
        encode(&json!({"text":"\u{0}".repeat(FRAME_BYTES)}))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
}
