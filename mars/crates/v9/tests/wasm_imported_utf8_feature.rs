/* The shell feature flag controls UTF-8 compile-time string imports. */
#![cfg(feature = "conformance")]
use crate::common;

/** Disabled collections remain ordinary imports; enabled collections resolve builtins. */
#[test]
fn utf8_imports_follow_shell_feature_state() {
    let source = "let name=s=>[s.length,...Array.from(s,c=>c.charCodeAt(0))],sec=(i,p)=>[i,p.length,...p],bytes=new Uint8Array([0,97,115,109,1,0,0,0,...sec(1,[1,96,1,111,1,127]),...sec(2,[1,...name('wasm:text-encoder'),...name('measureStringAsUTF8'),0,0])]);let r;try{new WebAssembly.Instance(new WebAssembly.Module(bytes,{builtins:['text-encoder']}),{});r='builtin'}catch(e){r=e instanceof TypeError?'type':e instanceof WebAssembly.LinkError?'link':'other'}";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let disabled = common::load_with(source, &environment);
    assert!(
        disabled.failed.is_empty(),
        "{:?}",
        common::failure(&disabled)
    );
    assert_eq!(common::global(&disabled, "r"), "type");
    environment.conformance_wasm_imported_strings_utf8 = true;
    let enabled = common::load_with(source, &environment);
    assert!(enabled.failed.is_empty(), "{:?}", common::failure(&enabled));
    assert_eq!(common::global(&enabled, "r"), "builtin");
}
