/** The URL Standard parser behind URL and URLSearchParams: file hosts and drive letters,
empty path segments, IPv4 and IPv6 hosts, opaque paths, the setters' state overrides, ASCII
`xn--` labels, blob origins, the statics, and the URL to URLSearchParams link. */
mod common;

/** Evaluate `expr` after `setup`. */
fn eval(setup: &str, expr: &str) -> String {
    common::eval(&format!("{setup};const __r=String({expr});"), "__r")
}

/** href of `new URL(input, base)`, or "fail". */
fn href(input: &str, base: &str) -> String {
    let base = if base.is_empty() {
        "undefined".to_string()
    } else {
        format!("{base:?}")
    };
    eval(
        &format!("let h;try{{h=new URL({input:?},{base}).href}}catch(e){{h='fail'}}"),
        "h",
    )
}

/** File URLs keep their host, their empty segments and Windows drive letters. */
#[test]
fn file_urls_follow_the_file_states() {
    assert_eq!(href("file:///w|/m", ""), "file:///w:/m");
    assert_eq!(href("file://spider///", ""), "file://spider///");
    assert_eq!(href("file:\\\\//", ""), "file:////");
    assert_eq!(href("C|/", "file://host/"), "file://host/C:/");
    assert_eq!(href("//localhost//pig", "file://lion/"), "file:////pig");
    assert_eq!(href("..", "file:///C:/"), "file:///C:/");
    assert_eq!(href("file://example.net/C:/", ""), "file://example.net/C:/");
    assert_eq!(href("file://example:1/", ""), "fail");
}

/** IPv4 parts in hex and octal, IPv6 compression, and opaque hosts. */
#[test]
fn hosts_parse_as_the_standard_says() {
    assert_eq!(href("http://0x7f.1/", ""), "http://127.0.0.1/");
    assert_eq!(href("http://0300.0250.0.1/", ""), "http://192.168.0.1/");
    assert_eq!(href("http://[0:0:0:0:0:0:0:1]/", ""), "http://[::1]/");
    assert_eq!(href("http://[1:0::]/", ""), "http://[1::]/");
    assert_eq!(href("http://[::127.0.0.1]/", ""), "http://[::7f00:1]/");
    assert_eq!(href("http://256.256.256.256/", ""), "fail");
    assert_eq!(href("sc://ñ/", ""), "sc://%C3%B1/");
    assert_eq!(href("https://xn--/", ""), "https://xn--/");
    assert_eq!(
        href("http://a.b.c.XN--pokxncvks", ""),
        "http://a.b.c.xn--pokxncvks/"
    );
    assert_eq!(href("http://\u{1f}lookout.net/", ""), "fail");
}

/** Opaque paths, dot segments and the `/.` serialization guard. */
#[test]
fn paths_and_opaque_paths() {
    assert_eq!(href("data:space ?query", ""), "data:space%20?query");
    assert_eq!(href("http://h/a/%2e%2E/b/./c", ""), "http://h/b/c");
    assert_eq!(
        href("web+demo:/.//not-a-host/", ""),
        "web+demo:/.//not-a-host/"
    );
    assert_eq!(href("#x", "mailto:a@b"), "mailto:a@b#x");
    assert_eq!(href("x", "mailto:a@b"), "fail");
}

/** Setters run the parser with a state override and leave the URL alone on failure. */
#[test]
fn setters_use_state_overrides() {
    let set = |url: &str, name: &str, value: &str| {
        eval(
            &format!("const u=new URL({url:?});u.{name}={value:?}"),
            "u.href",
        )
    };
    assert_eq!(set("http://a/", "protocol", "https"), "https://a/");
    assert_eq!(set("http://u@a/", "protocol", "file"), "http://u@a/");
    assert_eq!(set("https://a:443/", "port", "443"), "https://a/");
    assert_eq!(set("http://a/", "host", "b:81"), "http://b:81/");
    assert_eq!(set("file://y/", "host", "loc%41lhost"), "file:///");
    assert_eq!(set("http://a/", "pathname", "x y"), "http://a/x%20y");
    assert_eq!(
        set("javascript:alert(1)", "hash", "castle"),
        "javascript:alert(1)#castle"
    );
    assert_eq!(set("http://a/", "username", "me@"), "http://me%40@a/");
}

/** blob origins come from http(s) inner URLs only; the statics parse without throwing. */
#[test]
fn origins_and_statics() {
    let origin = |url: &str| eval(&format!("const u=new URL({url:?})"), "u.origin");
    assert_eq!(origin("blob:https://a.example/x"), "https://a.example");
    assert_eq!(origin("blob:ftp://host/path"), "null");
    assert_eq!(origin("file:///x"), "null");
    assert_eq!(
        eval("", "URL.canParse('a:b') + ',' + URL.canParse('nope')"),
        "true,false"
    );
    assert_eq!(
        eval(
            "",
            "URL.parse('/x', 'http://h/').href + ',' + URL.parse('nope')"
        ),
        "http://h/x,null"
    );
}

/** One URLSearchParams per URL, kept in step with `search` both ways. */
#[test]
fn search_params_are_linked_to_their_url() {
    let setup = "const u=new URL('http://h/?a=b');const p=u.searchParams;p.append('c','d e');const s1=u.search;u.search='?x=1';";
    assert_eq!(
        eval(
            setup,
            "[u.searchParams===p, s1, p.get('x'), (p.delete('x'), u.href)].join('|')"
        ),
        "true|?a=b&c=d+e|1|http://h/"
    );
}

/** sort is stable by UTF-16 code units; delete and has take an optional value; iteration is
live. */
#[test]
fn search_params_methods() {
    assert_eq!(
        eval(
            "const p=new URLSearchParams('z=1&\u{fffd}=2&\u{1f308}=3&z=0');p.sort()",
            "p.toString()"
        ),
        "z=1&z=0&%F0%9F%8C%88=3&%EF%BF%BD=2"
    );
    assert_eq!(
        eval(
            "const p=new URLSearchParams('a=1&a=2&b=3');p.delete('a','2')",
            "[p.toString(),p.has('b','3'),p.has('b','4')].join()"
        ),
        "a=1&b=3,true,false"
    );
    assert_eq!(
        eval(
            "const p=new URLSearchParams('p1=1&p2=2&p3=3');const seen=[];for(const [k] of p){seen.push(k);if(k==='p1')p.delete('p1')}",
            "seen.join()"
        ),
        "p1,p3"
    );
    assert_eq!(
        eval(
            "const p=new URLSearchParams({a:'1',b:'2'});const q=new URLSearchParams([['x','y']])",
            "p+'&'+q"
        ),
        "a=1&b=2&x=y"
    );
    assert_eq!(
        eval(
            "let e;try{new URLSearchParams([['a']])}catch(x){e=x.name}",
            "e"
        ),
        "TypeError"
    );
}

/** Invalid URL reporting must not run an input object's conversion a second time. */
#[test]
fn invalid_url_converts_input_once() {
    assert_eq!(
        eval(
            "let calls=0;let error;const input={toString(){calls++;return 'invalid'}};try{new URL(input)}catch(e){error=e.name}",
            "calls+','+error"
        ),
        "1,TypeError"
    );
}

/** Conversions can mutate the list, and the operation must use the resulting list. */
#[test]
fn search_params_read_live_pairs_after_argument_conversion() {
    assert_eq!(
        eval(
            "const p=new URLSearchParams('a=old');const key={toString(){p.set('a','new');return 'a'}};const read=p.get(key);const value={toString(){p.append('b','kept');return 'written'}};p.set('a',value)",
            "read+'|'+p"
        ),
        "new|a=written&b=kept"
    );
}

/** Ignored arguments and the callback receiver never undergo string conversion. */
#[test]
fn search_params_ignore_extra_arguments_and_preserve_callback_receiver() {
    assert_eq!(
        eval(
            "const p=new URLSearchParams('a=1');const receiver={toString(){throw new Error('coerced')}};const value=p.get('a',receiver);let same=false;p.forEach(function(){same=this===receiver},receiver)",
            "value+','+same"
        ),
        "1,true"
    );
}

/** An empty list still validates forEach's required callable argument. */
#[test]
fn search_params_empty_foreach_rejects_noncallable() {
    assert_eq!(
        eval(
            "let error;try{new URLSearchParams().forEach(null)}catch(e){error=e.name}",
            "error"
        ),
        "TypeError"
    );
}

/** Live iterators see appends until completion; a completed iterator remains done. */
#[test]
fn search_params_iterator_completion_is_permanent() {
    assert_eq!(
        eval(
            "const p=new URLSearchParams('a=1');const it=p.keys();const a=it.next().value;p.append('b','2');const b=it.next().value;const done=it.next().done;p.append('c','3')",
            "[a,b,done,it.next().done].join()"
        ),
        "a,b,true,true"
    );
}
