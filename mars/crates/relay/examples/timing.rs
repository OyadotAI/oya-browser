/** Times each phase of a Relay fetch, plus repeated end-to-end fetches through the public API. */
use mars_relay::{Prepared, WebRequest};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

/** Each public fetch gets the whole navigation budget. */
const REMAINING_MS: u64 = 8_000;

/** Repeated visits show what reuse and caching save after the first. */
const VISITS: usize = 4;

/** One socket read; matches the size Relay reads with. */
const READ_BYTES: usize = 16 * 1024;

/**
 * Usage: `timing <url> [fixture-origin]`; a loopback URL needs its origin as the grant.
 * Set MARS_TIMING_NO_CACHE to measure connection reuse and compression without the cache.
 */
fn main() {
    let mut args = std::env::args().skip(1);
    let url = args.next().expect("usage: timing <url> [fixture-origin]");
    let grants: Vec<String> = args.collect();
    // Unless MARS_TIMING_NO_CACHE is set, requests carry their own partition, as wired navigations will.
    let partitioned = std::env::var_os("MARS_TIMING_NO_CACHE").is_none();
    phases(&url);
    for visit in 0..VISITS {
        let start = Instant::now();
        let request = WebRequest {
            top_level: partitioned.then(|| url.clone()),
            ..WebRequest::get(&url)
        };
        let prepared = Prepared::connect(request, &grants, REMAINING_MS).expect("prepare");
        let connected = start.elapsed();
        let response = prepared.transmit().expect("transmit");
        println!(
            "relay visit {visit}: prepare {:.1} ms, transmit {:.1} ms, total {:.1} ms, status {}, {} bytes",
            ms(connected),
            ms(start.elapsed() - connected),
            ms(start.elapsed()),
            response.status,
            response.source.len()
        );
    }
}

/** A raw `Connection: close` exchange timed phase by phase, independently of Relay. */
fn phases(value: &str) {
    let url = url::Url::parse(value).expect("URL");
    let host = url
        .host_str()
        .expect("host")
        .trim_matches(['[', ']'])
        .to_owned();
    let port = url.port_or_known_default().expect("port");
    let start = Instant::now();
    let address = (host.as_str(), port)
        .to_socket_addrs()
        .expect("DNS")
        .next()
        .expect("answer");
    let dns = start.elapsed();
    let socket = TcpStream::connect(address).expect("connect");
    let connect = start.elapsed();
    let mut stream: Box<dyn ReadWrite> = if url.scheme() == "https" {
        Box::new(tls(socket, &host))
    } else {
        Box::new(socket)
    };
    let handshake = start.elapsed();
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept-Encoding: identity\r\n\r\n",
        &url[url::Position::BeforePath..url::Position::AfterQuery]
    );
    stream.write_all(request.as_bytes()).expect("write");
    let written = start.elapsed();
    let mut bytes = vec![0; READ_BYTES];
    let mut total = stream.read(&mut bytes).expect("first byte");
    let first = start.elapsed();
    while let Ok(count) = stream.read(&mut bytes) {
        if count == 0 {
            break;
        }
        total += count;
    }
    let body = start.elapsed();
    println!(
        "phases {value}: dns {:.1} ms, connect {:.1} ms, tls {:.1} ms, request {:.1} ms, first byte {:.1} ms, body {:.1} ms ({total} bytes)",
        ms(dns),
        ms(connect - dns),
        ms(handshake - connect),
        ms(written - handshake),
        ms(first - written),
        ms(body - first)
    );
}

/** The same verified Rustls configuration Relay uses, completed eagerly so it can be timed. */
fn tls(mut socket: TcpStream, host: &str) -> impl ReadWrite {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.to_owned()).expect("name");
    let mut connection = rustls::ClientConnection::new(Arc::new(config), name).expect("TLS client");
    while connection.is_handshaking() {
        connection.complete_io(&mut socket).expect("handshake");
    }
    rustls::StreamOwned::new(connection, socket)
}

/** Lets plaintext and TLS streams share one timed exchange. */
trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

/** Milliseconds with sub-millisecond precision. */
fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}
