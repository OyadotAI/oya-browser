/** FileReader reads Blobs into results, reports each read through its events, and refuses misuse as the File API specifies. */
use crate::common;

/** Loads a script, then runs the page's timers to a later virtual time, so the reads' queued tasks have fired; returns the named global. */
fn settled(source: &str, name: &str) -> String {
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let ran = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        loaded.state,
        100.0,
        64,
    )
    .unwrap();
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    common::global(&ran, name)
}

/** A data URL result carries the blob's type and base64 bytes; readyState and the loadend event land after the read. */
#[test]
fn data_url_read_settles_with_type_and_base64() {
    let source = "let result='';const r=new FileReader();r.addEventListener('loadend',()=>{result=[r.readyState,r.result].join('|')});r.readAsDataURL(new Blob(['hi'],{type:'text/plain'}));";
    assert_eq!(settled(source, "result"), "2|data:text/plain;base64,aGk=");
}

/** A blob without a type reads as application/octet-stream in a data URL, as the spec's default. */
#[test]
fn data_url_without_a_type_uses_the_octet_stream_default() {
    let source = "let result='';const r=new FileReader();r.onload=()=>{result=r.result};r.readAsDataURL(new Blob(['hi']));";
    assert_eq!(
        settled(source, "result"),
        "data:application/octet-stream;base64,aGk="
    );
}

/** Text results decode with the requested label, UTF-8 by default. */
#[test]
fn text_read_decodes_utf8_by_default() {
    let source = "let result='';const r=new FileReader();r.onload=()=>{result=r.result};r.readAsText(new Blob(['h\\u00e9llo']));";
    assert_eq!(settled(source, "result"), "h\u{e9}llo");
}

/** Binary string results carry one character per byte, and array buffer results carry the bytes. */
#[test]
fn binary_and_array_buffer_results_carry_the_bytes() {
    let source = "let binary='';let length=0;const a=new FileReader();a.onload=()=>{binary=a.result};a.readAsBinaryString(new Blob(['AB']));const b=new FileReader();b.onload=()=>{length=new Uint8Array(b.result)[1]};b.readAsArrayBuffer(new Blob(['AB']));let result='';setTimeout(()=>{result=binary+'|'+length},5);";
    assert_eq!(settled(source, "result"), "AB|66");
}

/** The events of a read arrive in the order the spec raises them: loadstart, load, then loadend. */
#[test]
fn a_read_raises_loadstart_load_and_loadend_in_order() {
    let source = "let log=[];const r=new FileReader();for(const t of ['loadstart','load','loadend'])r.addEventListener(t,()=>log.push(t));r.readAsText(new Blob(['x']));let result='';setTimeout(()=>{result=log.join(',')},5);";
    assert_eq!(settled(source, "result"), "loadstart,load,loadend");
}

/** Handler attributes run with the events they name, and a reader before any read is EMPTY. */
#[test]
fn handler_attributes_fire_and_the_state_constants_hold() {
    let source = "let result=[];const r=new FileReader();result.push(r.readyState,FileReader.EMPTY,FileReader.LOADING,FileReader.DONE);r.onloadend=()=>{result.push('end:'+r.readyState)};r.readAsText(new Blob(['x']));let text='';setTimeout(()=>{text=result.join(',')},5);";
    assert_eq!(settled(source, "text"), "0,0,1,2,end:2");
}

/** A read of a value that is not a Blob throws the TypeError the IDL conversion raises. */
#[test]
fn reading_a_non_blob_throws_a_type_error() {
    let source =
        "let result='';try{new FileReader().readAsText('not a blob')}catch(e){result=e.name}";
    assert_eq!(settled(source, "result"), "TypeError");
}

/** A second read while one is in progress throws InvalidStateError, and the first still settles. */
#[test]
fn a_second_read_while_busy_throws_invalid_state() {
    let source = "let result='';const r=new FileReader();r.readAsText(new Blob(['x']));try{r.readAsText(new Blob(['y']))}catch(e){result=e.name}setTimeout(()=>{result+='|'+r.result},5);";
    assert_eq!(settled(source, "result"), "InvalidStateError|x");
}

/** Abort during a read fires abort and loadend, never load, and clears the result. */
#[test]
fn abort_during_a_read_fires_abort_and_never_loads() {
    let source = "let log=[];const r=new FileReader();for(const t of ['load','abort','loadend'])r.addEventListener(t,()=>log.push(t));r.readAsText(new Blob(['x']));r.abort();let result='';setTimeout(()=>{result=log.join(',')+'|'+r.readyState+'|'+r.result},5);";
    assert_eq!(settled(source, "result"), "abort,loadend|2|null");
}

/** Abort on an idle reader changes nothing: no event, and the state stays EMPTY. */
#[test]
fn abort_on_an_idle_reader_is_a_no_op() {
    let source = "let log=[];const r=new FileReader();r.addEventListener('abort',()=>log.push('abort'));r.abort();let result=r.readyState+'|'+log.length;";
    assert_eq!(settled(source, "result"), "0|0");
}

/** FileReader is a constructor that must be called with new. */
#[test]
fn file_reader_requires_new() {
    let source = "let result='';try{FileReader()}catch(e){result=e.name}";
    assert_eq!(settled(source, "result"), "TypeError");
}
