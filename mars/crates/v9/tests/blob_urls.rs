/** Blob URLs retain branded immutable bytes only inside their creating VM context. */
use crate::common;
use mars_v9::{BlobUrlSource, WorkerCommand};

/** Inspect the captured source without acquiring anything from the network. */
fn source(command: &WorkerCommand) -> Option<&BlobUrlSource> {
    let WorkerCommand::Create { blob_source, .. } = command else {
        panic!("worker creation")
    };
    blob_source.as_ref()
}

#[test]
fn urls_are_unique_origin_bound_and_only_accept_branded_blobs() {
    let execution = common::load(
        "const blob=new Blob(['hello']);const a=URL.createObjectURL(blob);const b=URL.createObjectURL(blob);let fake='';try{URL.createObjectURL(Object.create(Blob.prototype))}catch(e){fake=e.name}const result=[a!==b,a.startsWith('blob:https://example.com/'),fake].join('|');",
    );
    assert_eq!(common::global(&execution, "result"), "true|true|TypeError");
    assert!(execution.failed.is_empty());
}

#[test]
fn worker_capture_preserves_bytes_mime_and_lifetime_across_revocation() {
    let execution = common::load(
        "const bytes=new Uint8Array([65,66]);const url=URL.createObjectURL(new Blob([bytes],{type:'TEXT/JAVASCRIPT'}));bytes[0]=90;new Worker(url+'#fragment');URL.revokeObjectURL(url);new Worker(url);",
    );
    assert!(execution.failed.is_empty());
    let captured = source(&execution.effects.worker_commands[0]).unwrap();
    assert_eq!(captured.bytes.as_slice(), b"AB");
    assert_eq!(captured.mime_type, "text/javascript");
    assert_eq!(captured.origin, "https://example.com");
    assert!(source(&execution.effects.worker_commands[1]).is_none());
}

#[test]
fn queries_do_not_alias_urls_and_unknown_revocation_is_a_noop() {
    let execution = common::load(
        "const url=URL.createObjectURL(new File(['42'],'worker.js'));URL.revokeObjectURL('blob:https://example.com/missing');new Worker(url+'?different');new Worker(url);URL.revokeObjectURL(url+'#fragment');new Worker(url);",
    );
    assert!(execution.failed.is_empty());
    assert!(source(&execution.effects.worker_commands[0]).is_none());
    assert!(source(&execution.effects.worker_commands[1]).is_some());
    assert!(source(&execution.effects.worker_commands[2]).is_none());
}

#[test]
fn an_url_string_does_not_transfer_its_creator_context_registry() {
    let execution = common::load("const url=URL.createObjectURL(new Blob(['postMessage(42)']));");
    let url = common::global(&execution, "url");
    let other = common::load(&format!(
        "new Worker({});",
        serde_json::to_string(&url).unwrap()
    ));
    assert!(other.failed.is_empty());
    assert!(source(&other.effects.worker_commands[0]).is_none());
}

#[test]
fn foreign_origin_urls_cannot_start_workers() {
    let execution = common::load(
        "let result='';try{new Worker('blob:https://attacker.test/12345678-1234-4123-8123-123456789abc')}catch(e){result=e.name}",
    );
    assert!(execution.failed.is_empty());
    assert_eq!(common::global(&execution, "result"), "SecurityError");
    assert!(execution.effects.worker_commands.is_empty());
}

#[test]
fn module_workers_capture_source_and_credentials_before_revocation() {
    let execution = common::load(
        "const url=URL.createObjectURL(new Blob(['export const answer=42'],{type:'TEXT/JAVASCRIPT; charset=UTF-8'}));new Worker(url,{type:'module',credentials:'omit'});URL.revokeObjectURL(url);",
    );
    assert!(execution.failed.is_empty());
    let command = &execution.effects.worker_commands[0];
    assert!(matches!(
        command,
        WorkerCommand::Create {
            worker_type: mars_v9::WorkerType::Module,
            credentials: mars_v9::WorkerCredentials::Omit,
            ..
        }
    ));
    assert_eq!(
        source(command).unwrap().mime_type,
        "text/javascript; charset=utf-8"
    );
}

#[test]
fn registry_and_source_validation_reject_noncanonical_or_oversized_records() {
    let execution =
        common::load("const url=URL.createObjectURL(new Blob(['42']));new Worker(url);");
    let captured = source(&execution.effects.worker_commands[0]).unwrap();
    let url = common::global(&execution, "url");
    assert!(captured.validate(&url).is_ok());
    assert!(
        captured
            .validate(&url.replace("example.com", "attacker.test"))
            .is_err()
    );
    assert!(captured.validate(&format!("{url}?different")).is_err());
    let mut malformed = captured.clone();
    malformed.mime_type = "TEXT/JAVASCRIPT".into();
    assert!(malformed.validate(&url).is_err());
    malformed = captured.clone();
    malformed.bytes = std::sync::Arc::new(vec![0; mars_core::constants::VM_STATE_BYTES / 16]);
    assert!(malformed.validate(&url).is_err());
    let mut restored = serde_json::to_value(execution.state).unwrap();
    restored["page"]["blob_urls"][&url]["origin"] = serde_json::json!("https://attacker.test");
    let restored: mars_v9::VmState = serde_json::from_value(restored).unwrap();
    assert!(restored.validate_document(&common::page()).is_err());
}

#[test]
fn restored_registry_retains_its_source_and_releases_it_when_revoked() {
    let script = "const url=URL.createObjectURL(new Blob(['42']));setTimeout(()=>{new Worker(url);URL.revokeObjectURL(url);new Worker(url)},1);";
    let execution = common::load(script);
    let host = common::host();
    let environment = common::environment();
    let sources = [script.to_string()];
    let input = common::input(&sources, &host, &environment);
    let result = mars_v9::run_timers(input, common::round_trip(execution.state), 1.0, 32).unwrap();
    assert!(result.failed.is_empty());
    assert!(source(&result.effects.worker_commands[0]).is_some());
    assert!(source(&result.effects.worker_commands[1]).is_none());
}
