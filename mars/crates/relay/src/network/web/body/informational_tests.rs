/** Buffered fixtures pin interim-head framing without sockets or server timing. */
use super::*;

/** A complete supplied response must never need another connection read. */
fn buffered(bytes: Vec<u8>) -> EngineResult<Response> {
    let mut read = |_: &mut [u8]| -> EngineResult<usize> {
        panic!("complete buffered fixture unexpectedly requested network input")
    };
    parsed_response(
        &mut Input {
            read: &mut read,
            bytes,
            position: 0,
        },
        false,
        false,
    )
}

/** Interim metadata is discarded while an already-buffered final body remains intact. */
#[test]
fn informational_heads_preserve_final_response_and_reuse() {
    let response = buffered(b"HTTP/1.1 103 Early Hints\r\nLink: </early.css>\r\nSet-Cookie: interim=1\r\n\r\nHTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 5\r\nSet-Cookie: final=1\r\n\r\nhello".to_vec()).unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body, b"hello");
    assert_eq!(response.set_cookies, ["final=1"]);
    assert!(!response.headers.contains_key("link"));
    assert!(response.reusable);
}

/** A final chunked response retains its framing after an interim head. */
#[test]
fn informational_heads_preserve_chunked_body() {
    let response = buffered(b"HTTP/1.1 103 Early Hints\r\n\r\nHTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n".to_vec()).unwrap();
    assert_eq!(response.body, b"hello");
    assert!(response.reusable);
}

/** The count bound accepts its edge and rejects one additional interim response. */
#[test]
fn informational_response_count_is_bounded() {
    let interim = "HTTP/1.1 103 Early Hints\r\n\r\n";
    let final_head = "HTTP/1.1 204 No Content\r\n\r\n";
    let accepted = format!("{}{final_head}", interim.repeat(INFORMATIONAL_RESPONSES));
    assert_eq!(buffered(accepted.into_bytes()).unwrap().status, 204);
    let refused = format!(
        "{}{final_head}",
        interim.repeat(INFORMATIONAL_RESPONSES + 1)
    );
    assert!(buffered(refused.into_bytes()).is_err());
}

/** Each head fitting individually cannot bypass the cumulative header-byte budget. */
#[test]
fn informational_and_final_heads_share_header_budget() {
    let value = "x".repeat(HEADER_BYTES / 2);
    let bytes = format!(
        "HTTP/1.1 103 Early Hints\r\nX-Padding: {value}\r\n\r\nHTTP/1.1 204 No Content\r\nX-Padding: {value}\r\n\r\n"
    );
    assert!(buffered(bytes.into_bytes()).is_err());
}

/** Upgrade negotiation is not mistaken for an ordinary informational response. */
#[test]
fn switching_protocols_is_refused_before_reading_upgrade_payload() {
    assert!(buffered(b"HTTP/1.1 101 Switching Protocols\r\n".to_vec()).is_err());
}
