/** Typed private frames must be byte-identical to the `Value` envelopes they replace. */
use mars::protocol::{
    EngineError, Request, Response, TypedResponse, decode, encode, encode_reply, encode_request,
};
use serde_json::json;

/** A typed request and a typed reply write exactly the bytes their `Value` forms write. */
#[test]
fn typed_frames_match_value_envelopes() {
    let params = json!({"source":"<p>\"x\"\n</p>","charset":null,"n":[1,2.5,{"k":true}]});
    let request = Request::new("7", "document.parse", params.clone());
    assert_eq!(
        encode_request("7", "document.parse", &params).unwrap(),
        encode(&request).unwrap()
    );
    let result = json!({"nodes":[{"tag":"p"}],"title":"t"});
    assert_eq!(
        encode_reply("7", &result).unwrap(),
        encode(&Response::new(Some("7".into()), Ok(result))).unwrap()
    );
}

/** Error replies and success replies both decode into the typed form. */
#[test]
fn typed_responses_read_both_shapes() {
    let error = encode(&Response::new(
        Some("3".into()),
        Err(EngineError::invalid("no")),
    ))
    .unwrap();
    let reply: TypedResponse<Vec<u32>> = decode(&error).unwrap();
    assert!(reply.result.is_none());
    assert_eq!(reply.error.unwrap().message, "no");
    let reply: TypedResponse<Vec<u32>> =
        decode(&encode_reply("3", &vec![1u32, 2]).unwrap()).unwrap();
    assert_eq!(reply.result, Some(vec![1, 2]));
    assert_eq!(reply.id.as_deref(), Some("3"));
}
