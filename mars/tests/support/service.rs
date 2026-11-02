/** Feature-gated fault executable; never part of the normal runtime build. */
use mars::constants::FRAME_BYTES;
use mars::protocol::{Request, Response, decode, read_frame, write_message};
use mars::{constants, protocol};
use serde_json::json;
use std::io::{self, BufReader, Write};
use std::path::PathBuf;
use std::time::Duration;
#[path = "../../src/runtime/lifetime.rs"]
mod lifetime;

/** The supervisor must kill this test stall long before it finishes. */
const STALL: Duration = Duration::from_secs(60);

/** Behavior comes only from the test-created executable name, not production input. */
fn main() {
    let executable = PathBuf::from(std::env::args().next().unwrap());
    let mode = executable.file_name().unwrap().to_str().unwrap();
    if mode == "owner_parent" {
        let mut child = std::process::Command::new(executable.with_file_name("owner_blocked"))
            .env_clear()
            .env(constants::OWNER_PID_ENV, std::process::id().to_string())
            .spawn()
            .unwrap();
        std::thread::sleep(STALL);
        let _ = child.wait();
        return;
    }
    if mode == "owner_blocked" {
        let _owner = lifetime::OwnerWatch::start().unwrap();
        println!("{}", std::process::id());
        io::stdout().flush().unwrap();
        std::thread::sleep(STALL);
        return;
    }
    let name = std::env::args().nth(2).unwrap();
    std::fs::write(
        executable.parent().unwrap().join("pid"),
        std::process::id().to_string(),
    )
    .unwrap();
    if mode == "startup_exit" {
        return;
    }
    let mut input = BufReader::new(io::stdin().lock());
    let mut output = io::stdout().lock();
    let ready: Request = decode(&read_frame(&mut input).unwrap().unwrap()).unwrap();
    write_message(
        &mut output,
        &Response::new(Some(ready.id), Ok(json!({"service":name,"ready":true}))),
    )
    .unwrap();
    fault(mode, &mut input, &mut output);
}

/** Each reachable failure is injected after a successful readiness handshake. */
fn fault(mode: &str, input: &mut impl std::io::BufRead, output: &mut impl Write) {
    if mode == "script_blocked" {
        script_blocked(input, output);
        return;
    }
    if mode == "blocked" {
        std::thread::sleep(STALL);
        return;
    }
    let Some(frame) = read_frame(input).unwrap() else {
        return;
    };
    let request: Request = decode(&frame).unwrap();
    if mode == "exit" {
        return;
    }
    if mode == "malformed" {
        output.write_all(b"not JSON\n").unwrap();
        return;
    }
    if mode == "oversized" {
        let _ = output.write_all(&vec![b'x'; FRAME_BYTES + 1]);
        return;
    }
    let mut response = Response::new(Some(request.id), Ok(json!(null)));
    if mode == "wrong_id" {
        response.id = Some("wrong".into());
    }
    if mode == "wrong_version" {
        response.version += 1;
    }
    write_message(output, &response).unwrap();
}

/** Serves an old page, then stalls a later external script request under the nav deadline. */
fn script_blocked(input: &mut impl std::io::BufRead, output: &mut impl Write) {
    let mut token = 0_u64;
    let mut current_url = String::new();
    while let Some(frame) = read_frame(input).unwrap() {
        let request: Request = decode(&frame).unwrap();
        let result = match request.op.as_str() {
            "network.prepare" => {
                current_url = request.params["request"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                if current_url.contains("stall.js") {
                    std::thread::sleep(STALL);
                    return;
                }
                if current_url.contains("first.js") {
                    std::thread::sleep(Duration::from_secs(6));
                }
                token += 1;
                Ok(json!({"token":token}))
            }
            "network.transmit" => Ok(script_response(&current_url)),
            _ => Err(protocol::EngineError::invalid(
                "Unexpected fault-test operation.",
            )),
        };
        write_message(output, &Response::new(Some(request.id), result)).unwrap();
    }
}

/** Synthetic network replies make the external-script failure path deterministic. */
fn script_response(url: &str) -> serde_json::Value {
    let (mime, source) = if url.contains("/healthy") {
        ("text/html", "<p id=title>previous</p>".to_owned())
    } else if url.contains("/stall") {
        (
            "text/html",
            "<p id=title>candidate</p><script src='/first.js'></script><script src='/stall.js?token=private-secret'></script>".to_owned(),
        )
    } else if url.contains("first.js") {
        ("text/javascript", "let ready=true;".to_owned())
    } else {
        ("text/html", "<p>unexpected</p>".to_owned())
    };
    json!({"url":url,"status":200,"source":source,"mime":mime,"location":null,"set_cookies":[]})
}
