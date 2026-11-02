/** Fetches one URL through Relay and prints the protocol and body, e.g. a TLS fingerprint echo. */
use mars_relay::{Prepared, WebRequest};

/** The whole navigation budget. */
const REMAINING_MS: u64 = 8_000;

/** Usage: `fetch <url>`. */
fn main() {
    let url = std::env::args().nth(1).expect("usage: fetch <url>");
    let prepared = Prepared::connect(WebRequest::get(&url), &[], REMAINING_MS).expect("connect");
    eprintln!("protocol {}", prepared.protocol());
    let response = prepared.transmit().expect("transmit");
    eprintln!("status {}", response.status);
    println!("{}", response.source);
}
