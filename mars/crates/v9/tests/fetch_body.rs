/** Fetch's "extract a body" and "consume body": Blob, FormData and object bodies, and the
arrayBuffer, bytes, blob and formData readers on Request and Response. */
use crate::common;
use common::{console, input};

/** Load one script and return the execution, with microtasks run. */
fn start(source: &str) -> mars_v9::Execution {
    let environment = common::environment();
    let host = common::host();
    mars_v9::load_page(input(&[source.to_string()], &host, &environment)).unwrap()
}

/** The content-type a recorded request carries. */
fn content_type(request: &mars_v9::HostRequest) -> Option<&str> {
    request
        .headers
        .iter()
        .find(|(name, _)| name == "content-type")
        .map(|(_, value)| value.as_str())
}

/** A Blob body sends its bytes, labelled with the Blob's type. */
#[test]
fn blob_body_carries_its_type() {
    let run = start("fetch('/x',{method:'POST',body:new Blob(['hi'],{type:'Text/X'})});");
    let request = &run.effects.requests[0];
    assert_eq!(request.body.as_deref(), Some("hi"));
    assert_eq!(content_type(request), Some("text/x"));
}

/** A FormData body is multipart with Blink's boundary, and the type names that boundary. */
#[test]
fn form_data_body_is_multipart() {
    let run = start(
        "var f=new FormData();f.append('a','1\\n2');f.append('f',new Blob(['z']),'n.txt');\
         fetch('/x',{method:'POST',body:f});",
    );
    let request = &run.effects.requests[0];
    let kind = content_type(request).unwrap();
    let boundary = kind.strip_prefix("multipart/form-data; boundary=").unwrap();
    assert!(boundary.starts_with("----WebKitFormBoundary") && boundary.len() == 38);
    let body = request.body.as_deref().unwrap();
    assert_eq!(
        body,
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n2\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"f\"; filename=\"n.txt\"\r\n\
             Content-Type: application/octet-stream\r\n\r\nz\r\n--{boundary}--\r\n"
        )
    );
}

/** Any other object is its string conversion, with no extracted type; an author type wins. */
#[test]
fn object_body_is_its_string() {
    let run = start(
        "var x=new XMLHttpRequest();x.open('POST','/x');\
         x.send({toString(){return 'obj'}});\
         fetch('/y',{method:'POST',headers:{'Content-Type':'a/b'},body:new Blob(['q'],{type:'c/d'})});",
    );
    assert_eq!(run.effects.requests[0].body.as_deref(), Some("obj"));
    assert_eq!(content_type(&run.effects.requests[1]), Some("a/b"));
}

/** XHR send() on GET drops the body without converting it. */
#[test]
fn xhr_get_sends_no_body() {
    let run = start(
        "var x=new XMLHttpRequest();x.open('GET','/x');\
         x.send({toString(){console.log('converted');return 'b'}});",
    );
    assert_eq!(run.effects.requests[0].body, None);
    assert!(console(&run).is_empty());
}

/** Response and Request read their body as an ArrayBuffer, bytes, a typed Blob and FormData. */
#[test]
fn body_readers_cover_every_kind() {
    let run = start(
        "(async()=>{var r=new Response('abc',{headers:{'content-type':'Text/Plain'}});\
         var b=await r.blob();console.log(b.size,b.type);\
         console.log((await new Response('ab').arrayBuffer()).byteLength);\
         console.log((await new Response('ab').bytes())[1]);\
         var q=new Request('/x',{method:'POST',body:new URLSearchParams('a=1&b=2')});\
         console.log((await q.formData()).get('b'));\
         console.log((await new Request('/x',{method:'POST',body:'xyz'}).arrayBuffer()).byteLength);\
         await new Response('x').formData().catch(e=>console.log(e instanceof TypeError));})();",
    );
    assert_eq!(console(&run), ["3 text/plain", "2", "98", "2", "3", "true"]);
}

/** RequestInit's duplex is a WebIDL enum whose only value is "half". */
#[test]
fn duplex_full_is_a_type_error() {
    let run = start(
        "try{new Request('/x',{method:'POST',body:new Blob(['a']),duplex:'full'})}\
         catch(e){console.log(e instanceof TypeError)}\
         console.log(new Request('/x',{method:'POST',body:'a',duplex:'half'}).method);",
    );
    assert_eq!(console(&run), ["true", "POST"]);
}

/** A multipart body round-trips through formData(): text entries and a file with its name. */
#[test]
fn multipart_body_reads_back_as_form_data() {
    let run = start(
        "(async()=>{var f=new FormData();f.append('a','1');f.append('f',new Blob(['zz'],{type:'x/y'}),'n.txt');\
         var g=await new Request('/x',{method:'POST',body:f}).formData();\
         var file=g.get('f');console.log(g.get('a'),file.name,file.type,await file.text());\
         await new Response('junk',{headers:{'content-type':'multipart/form-data; boundary=b'}})\
         .formData().catch(e=>console.log(e instanceof TypeError));})();",
    );
    assert_eq!(console(&run), ["1 n.txt x/y zz", "true"]);
}

/** A close delimiter not followed by CRLF or the end is a malformed body (WPT "buggy form
data - empty"), while an empty FormData round-trips. */
#[test]
fn multipart_close_must_end_its_line() {
    let run = start(
        "(async()=>{var h={headers:{'content-type':'multipart/form-data; boundary=b'}};\
         await new Response('--b--b--\\r\\n',h).formData().catch(e=>console.log('bad',e instanceof TypeError));\
         console.log([...(await new Response('--b--\\r\\n',h).formData()).keys()].length);})();",
    );
    assert_eq!(console(&run), ["bad true", "0"]);
}
