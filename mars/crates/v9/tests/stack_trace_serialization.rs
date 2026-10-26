/** Default stack serialization uses real CallSite source coordinates and distinguishes named functions from unnamed locations. */
mod common;

/** Source identity is supplied through the same public metadata path used by a real host. */
fn eval(source: &str) -> String {
    let mut environment = common::environment();
    environment.scripts = vec!["https://fixtures.example/stack.js".to_owned()];
    let ran = common::load_with(source, &environment);
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert!(
        ran.effects
            .console
            .iter()
            .all(|entry| entry.level != "error"),
        "{:?}",
        common::console(&ran)
    );
    common::global(&ran, "r")
}

/** Two captures from the same function instruction prove the printed coordinates without guessing column offsets. */
#[test]
fn named_frames_serialize_their_actual_source_location() {
    assert_eq!(
        eval(
            "Error.stackTraceLimit=1;function named(){return new Error('x')}const first=named(),second=named();const rendered=first.stack.split('\\n')[1].trim();Error.prepareStackTrace=(_,frames)=>frames;const frame=second.stack[0];const frameLocation=frame.getFileName()+':'+frame.getLineNumber()+':'+frame.getColumnNumber();let r=(frame.getFunction()===named)+','+(frame.getFunctionName()==='named')+','+(rendered==='at named ('+frameLocation+')');"
        ),
        "true,true,true"
    );
}

/** An actual unnamed callback has no fabricated function label; its default frame consists of the real location. */
#[test]
fn anonymous_frames_serialize_a_bare_source_location() {
    assert_eq!(
        eval(
            "Error.stackTraceLimit=1;const errors=[0,1].map(function(){return new Error('x')});const rendered=errors[0].stack.split('\\n')[1].trim();Error.prepareStackTrace=(_,frames)=>frames;const frame=errors[1].stack[0];const frameLocation=frame.getFileName()+':'+frame.getLineNumber()+':'+frame.getColumnNumber();let r=(frame.getFunctionName()===null)+','+(typeof frame.getFunction()==='function')+','+(rendered==='at '+frameLocation);"
        ),
        "true,true,true"
    );
}

/** An async prefix denotes a genuinely reconstructed continuation and uses that saved frame's own source position. */
#[test]
fn reconstructed_frames_serialize_the_actual_async_marker() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,frames)=>frames;async function parent(){return await leaf()}async function leaf(){await 0;return new Error('x').stack}let r;parent().then(frames=>{const frame=frames[1];const frameLocation=frame.getFileName()+':'+frame.getLineNumber()+':'+frame.getColumnNumber();r=frame.isAsync()+','+(frame.getFunction()===parent)+','+(frame.toString()==='async parent ('+frameLocation+')')});"
        ),
        "true,true,true"
    );
}
