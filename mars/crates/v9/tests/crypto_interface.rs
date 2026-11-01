/** The Crypto interface: crypto is a real Crypto instance, its methods live on the prototype, and its construction is illegal. */
use crate::common;
use mars_v9::{DocumentHost, Execution};

/** Run a script in a default page and return its global `r`. */
fn run(script: &str) -> String {
    let host = DocumentHost { ..common::host() };
    let environment = common::environment();
    let sources = vec![script.to_string()];
    let execution: Execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&execution), None);
    common::global(&execution, "r")
}

/** crypto is an instance of Crypto, and Object.prototype.toString names it by its interface. */
#[test]
fn crypto_is_an_instance_of_the_crypto_interface() {
    assert_eq!(
        run(
            "const r=[crypto instanceof Crypto,Object.prototype.toString.call(crypto),typeof Crypto].join('|');"
        ),
        "true|[object Crypto]|function"
    );
}

/** The methods are inherited from the prototype, so the instance owns none of them. */
#[test]
fn crypto_methods_live_on_the_prototype() {
    assert_eq!(
        run(
            "const r=[Object.hasOwn(crypto,'getRandomValues'),Object.hasOwn(crypto,'randomUUID'),Object.hasOwn(Crypto.prototype,'getRandomValues'),Object.hasOwn(Crypto.prototype,'randomUUID')].join('|');"
        ),
        "false|false|true|true"
    );
}

/** Script cannot construct a Crypto, and the subtle getter answers on the instance. */
#[test]
fn crypto_cannot_be_constructed_and_subtle_is_a_subtle_crypto() {
    assert_eq!(
        run(
            "let e='none';try{new Crypto()}catch(x){e=x.name}const r=e+'|'+(crypto.subtle instanceof SubtleCrypto);"
        ),
        "TypeError|true"
    );
}

/** A random UUID comes from the prototype method and has the version-4 shape. */
#[test]
fn random_uuid_is_a_version_four_identifier() {
    assert_eq!(
        run(
            "const r=/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(crypto.randomUUID())+'';"
        ),
        "true"
    );
}
