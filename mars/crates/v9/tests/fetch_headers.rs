/** Fetch's Headers, Request headers and Response constructor semantics (ported from Blink's
Headers, FetchHeaderList and Response): validation, normalisation, guards, sort-and-combine and
the Response statics. Each test fails on the old bindings. */
mod common;
use common::{console, input};

/** Run one script and return what it logged. */
fn run(script: &str) -> Vec<String> {
    let environment = common::environment();
    let host = common::host();
    let sources = vec![script.to_string()];
    console(&mars_v9::load_page(input(&sources, &host, &environment)).unwrap())
}

/** Whether `code` throws, and the error's constructor name. */
fn throws(code: &str) -> String {
    format!("try{{{code};console.log('none')}}catch(e){{console.log(e.constructor.name)}}")
}

/** Invalid names and values are TypeErrors, as are init pairs that are not two items. */
#[test]
fn invalid_names_and_values_throw() {
    let script = [
        throws("new Headers([['name']])"),
        throws("new Headers([['a','b','c']])"),
        throws("new Headers([['bad name','v']])"),
        throws("new Headers().get('invalid\\u0100')"),
        throws("new Headers().set('name','invalid\\u0100')"),
        throws("new Headers().append('name','a\\nb')"),
        throws("new Headers('nope')"),
    ]
    .join("");
    assert_eq!(run(&script), ["TypeError"; 7]);
}

/** Values lose leading and trailing HTTP whitespace only. */
#[test]
fn values_are_normalized() {
    let out = run(
        "var h=new Headers({a:' x ',b:'\\r\\n\\ty\\t',c:'z\\u00a0'});\
                   console.log(JSON.stringify([h.get('a'),h.get('b'),h.get('c')]))",
    );
    assert_eq!(out, ["[\"x\",\"y\",\"z\u{a0}\"]"]);
}

/** Iteration is sorted by name and combined, but each Set-Cookie stays its own entry. */
#[test]
fn iteration_sorts_and_combines_except_set_cookie() {
    let out = run(
        "var h=new Headers([['b','1'],['a','2'],['b','3'],['set-cookie','x'],['set-cookie','y']]);\
                   console.log(JSON.stringify([...h]));console.log(JSON.stringify(h.getSetCookie()))",
    );
    assert_eq!(
        out,
        [
            "[[\"a\",\"2\"],[\"b\",\"1, 3\"],[\"set-cookie\",\"x\"],[\"set-cookie\",\"y\"]]",
            "[\"x\",\"y\"]"
        ]
    );
}

/** set replaces the first entry in place and removes the rest; forEach needs a callable. */
#[test]
fn set_and_foreach_follow_the_spec() {
    let out = run(&format!(
        "var h=new Headers([['a','1'],['b','2'],['a','3']]);h.set('a','9');\
         console.log(JSON.stringify([...h]));{}",
        throws("h.forEach(1)")
    ));
    assert_eq!(out, ["[[\"a\",\"9\"],[\"b\",\"2\"]]", "TypeError"]);
}

/** A Request's headers drop forbidden names, a no-cors one keeps only safelisted ones, and
`request.headers` is one live object. */
#[test]
fn request_guards_drop_forbidden_headers() {
    let out = run(
        "var r=new Request('/x',{headers:{cookie:'a=b','x-ok':'1'}});\
         console.log(r.headers.has('cookie'),r.headers.get('x-ok'),r.headers===r.headers);\
         r.headers.set('host','evil');r.headers.set('x-two','2');\
         console.log(r.headers.has('host'),r.headers.get('x-two'));\
         var n=new Request('/x',{mode:'no-cors',headers:{'x-custom':'1',accept:'text/html'}});\
         console.log(n.headers.has('x-custom'),n.headers.get('accept'))",
    );
    assert_eq!(out, ["false 1 true", "false 2", "false text/html"]);
}

/** new Response validates status and statusText, labels text bodies, and drops Set-Cookie;
error() and redirect() build immutable responses. */
#[test]
fn response_constructor_and_statics() {
    let script = [
        "var r=new Response('hi',{status:201,statusText:'Made',headers:{'set-cookie':'a','x':'1'}});".to_string(),
        "console.log(r.status,r.statusText,r.type,r.headers.get('content-type'),r.headers.has('set-cookie'));".into(),
        "r.text().then(t=>console.log(t));".into(),
        throws("new Response('',{status:600})"),
        throws("new Response('',{statusText:'\\n'})"),
        throws("new Response('x',{status:204})"),
        "var e=Response.error();console.log(e.type,e.status);".into(),
        throws("e.headers.set('a','b')"),
        "var d=Response.redirect('https://example.com/next',301);console.log(d.status,d.headers.get('location'));".into(),
        throws("Response.redirect('https://example.com/',200)"),
    ]
    .join("");
    assert_eq!(
        run(&script),
        [
            "201 Made default text/plain;charset=UTF-8 false",
            "RangeError",
            "TypeError",
            "TypeError",
            "error 0",
            "TypeError",
            "301 https://example.com/next",
            "RangeError",
            "hi",
        ]
    );
}

/** The Request constructor refuses what Fetch refuses, and normalises only the standard
methods. */
#[test]
fn request_constructor_validates_init() {
    let script = [
        throws("new Request('/x',{method:'TRACE'})"),
        throws("new Request('/x',{method:'IN VALID'})"),
        throws("new Request('https://u:p@example.com/')"),
        throws("new Request('/x',{mode:'navigate'})"),
        throws("new Request('/x',{mode:'no-cors',method:'PUT'})"),
        throws("new Request('/x',{cache:'only-if-cached'})"),
        throws("new Request('/x',{redirect:'BAD'})"),
        throws("new Request('/x',{window:{}})"),
        throws("new Request('/x',{body:'a'})"),
        throws("Request('/x')"),
        "console.log(new Request('/x',{method:'patch'}).method,new Request('/x',{method:'post'}).method);".into(),
        "console.log(new Request('/x',{method:'POST',body:'a'}).headers.get('content-type'))".into(),
    ]
    .join("");
    let mut expected = vec!["TypeError"; 10];
    expected.extend(["patch POST", "text/plain;charset=UTF-8"]);
    assert_eq!(run(&script), expected);
}

/** Response.json serialises its data and labels it application/json. */
#[test]
fn response_json_labels_and_serializes() {
    let script = format!(
        "var r=Response.json({{a:1}},{{status:202}});console.log(r.status,r.headers.get('content-type'));\
         r.text().then(t=>console.log(t));{}",
        throws("Response.json(1n)")
    );
    assert_eq!(
        run(&script),
        ["202 application/json", "TypeError", "{\"a\":1}"]
    );
}
