/** A standalone Relay consumer validates a request without DNS, sockets or application startup. */
use mars_relay::{WebRequest, policy};

/** Validation is explicit; only Prepared::connect followed by transmit performs a request. */
fn main() {
    let request = WebRequest::get("https://1.1.1.1/document");
    request.validate().expect("bounded GET");
    let url = policy::web_url(&request.url).expect("allowed URL syntax");
    assert!(policy::public_address(
        "1.1.1.1".parse().expect("literal public IP")
    ));
    assert!(policy::web_url("https://user:secret@1.1.1.1/").is_err());
    println!("Relay validated {} {}", url.scheme(), url.path());
}
