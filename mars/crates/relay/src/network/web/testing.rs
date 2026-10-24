/** Loopback TLS for unit tests: a test CA, and a scripted server with a handshaken client. */
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName};
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, StreamOwned,
};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;

/** The test CA, leaf certificate and key, valid until 2126, for `localhost` and 127.0.0.1. */
const CA: &[u8] = include_bytes!("fixtures/ca.der");
const LEAF: &[u8] = include_bytes!("fixtures/leaf.der");
const KEY: &[u8] = include_bytes!("fixtures/leaf.key.der");

/// The server side of a test connection.
pub(super) type ServerStream = StreamOwned<ServerConnection, TcpStream>;

/// The client side of a test connection, as `transport::Connection::Tls` holds it.
pub(super) type ClientStream = StreamOwned<ClientConnection, TcpStream>;

/// Starts a one-connection TLS server offering `alpn` that runs `script`, and returns a client
/// that trusts the test CA and has completed its handshake with it.
pub(super) fn tls(
    alpn: &[u8],
    script: impl FnOnce(ServerStream) + Send + 'static,
) -> (ClientStream, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let protocol = alpn.to_vec();
    let server = std::thread::spawn(move || {
        let (socket, _) = listener.accept().expect("accept");
        let mut config =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .expect("versions")
                .with_no_client_auth()
                .with_single_cert(
                    vec![CertificateDer::from(LEAF.to_vec())],
                    PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(KEY.to_vec())),
                )
                .expect("server config");
        config.alpn_protocols = vec![protocol];
        let connection = ServerConnection::new(Arc::new(config)).expect("server");
        script(StreamOwned::new(connection, socket));
    });
    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from(CA.to_vec())).expect("root");
    let mut config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("versions")
            .with_root_certificates(roots)
            .with_no_client_auth();
    config.alpn_protocols = vec![alpn.to_vec()];
    let name = ServerName::try_from("localhost").expect("name");
    let socket = TcpStream::connect(address).expect("connect");
    let mut client = StreamOwned::new(
        ClientConnection::new(Arc::new(config), name).expect("client"),
        socket,
    );
    while client.conn.is_handshaking() {
        client
            .conn
            .complete_io(&mut client.sock)
            .expect("handshake");
    }
    (client, server)
}
