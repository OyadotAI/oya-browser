/**
 * The platform pieces YouTube and Google sign-in needed: Range and Selection, Intl, reads of
 * `super.x`, blank text nodes inserted into the document, `Node` constants, NamedNodeMap
 * lookups, `detail` on a plain Event, style sheets, validity, inserted script loading and the
 * input events typing fires. Each test pins one rule through the public entry points.
 */
use crate::common;

use common::{console, eval, failure, host, input, load_with, round_trip};
use mars_v9::{DomLevel, ModuleInput, ModuleSource, PageEvent};

/** Range boundaries, collapse, text and the comparison modes. */
#[test]
fn range_reports_boundaries_text_and_order() {
    let r = eval(
        "const p = document.getElementById('first'); const t = document.createTextNode('hello world');
         p.appendChild(t); const range = document.createRange(); range.selectNodeContents(p);
         range.setStart(t, 6); range.setEnd(t, 11);
         const other = new Range(); other.setStart(t, 0); other.setEnd(t, 2);
         const r = [range.toString(), range.collapsed, range.startOffset, range.endOffset,
           range.commonAncestorContainer === t, range.compareBoundaryPoints(Range.START_TO_START, other),
           Range.END_TO_END, range.cloneRange().toString(), range.isPointInRange(t, 7)].join('|');
         range.collapse(true); const r2 = range.collapsed;",
        "r",
    );
    assert_eq!(r, "world|false|6|11|true|1|2|world|true");
}

/** `getSelection` is one object whose ranges scripts add and clear. */
#[test]
fn selection_holds_the_added_range() {
    let r = eval(
        "const s = window.getSelection(); const same = s === document.getSelection();
         const p = document.getElementById('first'); p.appendChild(document.createTextNode('text'));
         const range = document.createRange(); range.selectNodeContents(p);
         s.addRange(range);
         const r = [same, s.rangeCount, s.type, s.getRangeAt(0) === range, s.anchorNode === range.startContainer].join('|');
         s.removeAllRanges(); const after = s.rangeCount;",
        "r",
    );
    assert_eq!(r, "true|1|Range|true|true");
}

/** Intl formats numbers, dates, plurals and lists the way en-US does. */
#[test]
fn intl_formats_in_english() {
    let r = eval(
        "const r = [new Intl.NumberFormat('en-u-nu-latn').format(1234567.891),
           new Intl.NumberFormat('en', {minimumIntegerDigits: 2}).format(7),
           new Intl.NumberFormat('en', {style: 'percent'}).format(0.256),
           new Intl.NumberFormat('en', {style: 'currency', currency: 'USD'}).format(1234.5),
           new Intl.NumberFormat('en', {notation: 'compact'}).format(1234567),
           new Intl.DateTimeFormat('en-US', {timeZone: 'UTC'}).format(new Date(Date.UTC(2024, 0, 5, 15, 4, 9))),
           new Intl.DateTimeFormat('en-US', {month: 'short', day: 'numeric', year: 'numeric', timeZone: 'UTC'}).format(new Date(Date.UTC(2024, 0, 5))),
           new Intl.PluralRules('en').select(1), new Intl.PluralRules('en', {type: 'ordinal'}).select(22),
           new Intl.RelativeTimeFormat('en', {numeric: 'auto'}).format(-1, 'day'),
           new Intl.ListFormat('en').format(['a', 'b', 'c']),
           new Intl.Collator('en').compare('a', 'B'),
           new Intl.DateTimeFormat().resolvedOptions().locale,
           Object.prototype.toString.call(new Intl.NumberFormat())].join('|');",
        "r",
    );
    assert_eq!(
        r,
        "1,234,567.891|07|26%|$1,234.50|1.2M|1/5/2024|Jan 5, 2024|one|two|yesterday|a, b, and c|-1|en-US|[object Intl.NumberFormat]"
    );
}

/** A read of `super.x` inside method `x` reaches the parent, not the method itself. */
#[test]
fn super_reads_go_through_the_home_object() {
    let r = eval(
        "class P {} class Q extends P { connectedCallback() { return typeof super.connectedCallback } }
         const mixin = base => class extends base { foo() { return 'M>' + (super.foo ? super.foo() : 'none') } };
         class A { foo() { return 'A' } }
         const r = [new Q().connectedCallback(), new (mixin(A))().foo(), new (mixin(P))().foo()].join('|');",
        "r",
    );
    assert_eq!(r, "undefined|M>A|M>none");
}

/** A blank text node appended to a projected element stays a node a script can fill later. */
#[test]
fn blank_text_nodes_survive_insertion() {
    let mut environment = common::environment();
    environment.dom_level = DomLevel::Extended;
    let execution = load_with(
        "const p = document.getElementById('first'); const t = document.createTextNode('');
         p.appendChild(t); const before = p.childNodes.length; t.textContent = 'filled';
         const r = [before, p.textContent, t.parentNode === p, t.data].join('|');",
        &environment,
    );
    assert_eq!(failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "1|filled|true|filled");
}

/**
 * Attaching a detached subtree keeps every text node it holds as the same node: empty texts a
 * script will fill, and texts next to each other, which markup alone would merge into one.
 */
#[test]
fn attached_subtree_keeps_empty_and_adjacent_text_nodes() {
    let mut environment = common::environment();
    environment.dom_level = DomLevel::Extended;
    let execution = load_with(
        "const host = document.getElementById('first'); const d = document.createElement('div');
         d.appendChild(document.createTextNode('')); d.appendChild(document.createTextNode(''));
         d.appendChild(document.createTextNode('x')); d.appendChild(document.createTextNode('y'));
         d.appendChild(document.createElement('b')); const made = Array.from(d.childNodes);
         host.appendChild(d); const kids = Array.from(d.childNodes);
         const r = [kids.length, kids.map(n => n.data ?? n.tagName).join(','),
           kids.every((n, i) => n === made[i]), made.every(n => n.parentNode === d)].join('|');",
        &environment,
    );
    assert_eq!(failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "5|,,x,y,B|true|true");
}

/** `Node.ELEMENT_NODE` lives on the constructor as well as the prototype. */
#[test]
fn node_constants_are_static() {
    assert_eq!(
        eval(
            "const r = [Node.ELEMENT_NODE, Node.TEXT_NODE, Node.DOCUMENT_POSITION_FOLLOWING, Node.prototype.COMMENT_NODE].join('|');",
            "r"
        ),
        "1|3|4|8"
    );
}

/** `attributes` answers `getNamedItem` and `item`, and `detail` is assignable on an Event. */
#[test]
fn attributes_map_and_event_detail() {
    let r = eval(
        "'use strict'; const d = document.createElement('div'); d.setAttribute('class-name', 'x');
         const map = d.attributes; const e = new Event('fire'); e.detail = {n: 1};
         const r = [map.length, map.getNamedItem('class-name').value, map.item(0).name, String(map.item(5)), e.detail.n,
           new CustomEvent('c', {detail: 3}).detail].join('|');",
        "r",
    );
    assert_eq!(r, "1|x|class-name|null|1|3");
}

/** `document.styleSheets` lists sheets whose rules come from the element's text. */
#[test]
fn style_sheets_follow_style_elements() {
    let r = eval(
        "const s = document.createElement('style'); s.textContent = 'p { color: red } .a { margin: 0 }';
         document.head.appendChild(s); const sheet = document.styleSheets[0];
         const n = sheet.insertRule('b { x: 1 }', 2);
         const i = document.createElement('input');
         const r = [document.styleSheets.length, sheet.ownerNode === s, s.sheet.cssRules.length, s.sheet.cssRules[0].selectorText, n,
           i.validity.valid, i.validity.badInput, i.willValidate].join('|');",
        "r",
    );
    assert_eq!(r, "1|true|3|p|2|true|false|true");
}

/** A `<script src>` a script connects asks the app for its source, runs it once delivered, and
fires `load` on the element; a refused one fires `error`. */
#[test]
fn inserted_scripts_load_through_the_app() {
    let environment = common::environment();
    let sources = vec![
        "let loaded = 'no'; let errored = 'no';
         const s = document.createElement('script'); s.src = '/chunk.js'; s.setAttribute('nonce', 'n1');
         s.addEventListener('load', () => { loaded = 'yes' }); document.head.appendChild(s);
         const bad = document.createElement('script'); bad.src = '/missing.js';
         bad.addEventListener('error', () => { errored = 'yes' }); document.head.appendChild(bad);
         const inline = document.createElement('script'); inline.textContent = 'window.inlineRan = 1';
         document.head.appendChild(inline); const inlineRan = window.inlineRan;"
            .to_string(),
    ];
    let execution = load_with(&sources[0], &environment);
    assert_eq!(failure(&execution), None);
    let urls: Vec<&str> = execution
        .script_requests
        .iter()
        .map(|request| request.url.as_str())
        .collect();
    assert_eq!(
        urls,
        [
            "https://example.com/chunk.js",
            "https://example.com/missing.js"
        ]
    );
    assert_eq!(execution.script_requests[0].nonce.as_deref(), Some("n1"));
    assert_eq!(common::global(&execution, "inlineRan"), "1");
    let modules = ModuleInput {
        entries: Vec::new(),
        sources: vec![
            ModuleSource {
                url: "https://example.com/chunk.js".into(),
                source: Some("const ran = 'chunk'".into()),
                classic: true,
            },
            ModuleSource {
                url: "https://example.com/missing.js".into(),
                source: None,
                classic: true,
            },
        ],
    };
    let host = host();
    let mut page_input = input(&sources, &host, &environment);
    page_input.modules = &modules;
    let after = mars_v9::load_modules(page_input, round_trip(execution.state)).expect("delivered");
    assert_eq!(common::global(&after, "ran"), "chunk");
    assert_eq!(common::global(&after, "loaded"), "yes");
    assert_eq!(common::global(&after, "errored"), "yes");
    assert!(after.script_requests.is_empty());
    // The delivered script's lexical global survives the next request's validation.
    let mut again = input(&sources, &host, &environment);
    again.modules = &modules;
    let next = mars_v9::page_event(again, round_trip(after.state), PageEvent::Load).expect("next");
    assert_eq!(common::global(&next, "ran"), "chunk");
}

/** Typing into a field fires trusted `input` and `change` events the page's listeners see. */
#[test]
fn typing_fires_input_and_change() {
    let environment = common::environment();
    let sources = vec![
        "let seen = ''; const go = document.getElementById('go');
         go.addEventListener('input', e => { seen += 'input:' + e.isTrusted + ':' + go.value });
         document.addEventListener('change', e => { seen += '|change:' + e.target.id });"
            .to_string(),
    ];
    let execution = load_with(&sources[0], &environment);
    assert_eq!(failure(&execution), None);
    let host = host();
    let after = mars_v9::input_page(
        input(&sources, &host, &environment),
        round_trip(execution.state),
        3,
        Some("abc".into()),
    )
    .expect("input events");
    assert_eq!(common::global(&after, "seen"), "input:true:abc|change:go");
    assert!(console(&after).is_empty());
}

/** Errors carry the activations they were made in, innermost first. */
#[test]
fn error_stack_names_the_frames() {
    let r = eval(
        "function inner() { throw new Error('boom') } function outer() { inner() }
         let r = ''; try { outer() } catch (e) { r = e.stack.split('\\n').map(l => l.trim().split(' (')[0]).join('|') }",
        "r",
    );
    // Top-level code is named by the inline script's document, without its query.
    assert_eq!(
        r,
        "Error: boom|at inner|at outer|at https://example.com/app/page:2:28"
    );
}

/** Committed state crosses the pipe as JSON text; two Map keys a lossy float parser would read
as one double (2186694896461479.25 prints as "…479.2") must come back distinct. */
#[test]
fn state_text_keeps_distinct_float_keys() {
    let execution = common::load(
        "const m = new Map(); m.set(2186694896461479.25, 'a'); m.set(2186694896461479.0, 'b');
         const r = m.size;",
    );
    assert_eq!(failure(&execution), None);
    let text = serde_json::to_string(&execution.state).expect("state encodes");
    let state: mars_v9::VmState = serde_json::from_str(&text).expect("distinct keys decode");
    let again = serde_json::to_string(&state).expect("state encodes again");
    assert_eq!(again.matches("2186694896461479").count(), 2);
}

/** `focus()` moves `document.activeElement` and fires the focus events; `blur()` clears it. */
#[test]
fn focus_moves_active_element() {
    let r = eval(
        "let seen = ''; const go = document.getElementById('go');
         go.addEventListener('focus', () => { seen += 'focus' }); go.addEventListener('blur', () => { seen += '|blur' });
         document.addEventListener('focusin', () => { seen += '|focusin' });
         const before = document.activeElement === document.body; go.focus();
         const during = document.activeElement === go; go.blur();
         const r = [before, during, document.activeElement === document.body, seen].join('|');",
        "r",
    );
    assert_eq!(r, "true|true|true|focus|focusin|blur");
}

/** `new Request` keeps what fetch would send, and `fetch(request)` sends exactly that. */
#[test]
fn request_objects_feed_fetch() {
    let execution = common::load(
        "const req = new Request('/api?x=1', {method: 'POST', body: 'a=1', headers: {'X-Test': 'y'}, credentials: 'include'});
         const r = [req.url, req.method, req.headers.get('x-test'), req.credentials, req.clone().url, Object.prototype.toString.call(req)].join('|');
         fetch(req); fetch(new Request('https://example.com/other'), {method: 'PUT'});",
    );
    assert_eq!(failure(&execution), None);
    assert_eq!(
        common::global(&execution, "r"),
        "https://example.com/api?x=1|POST|y|include|https://example.com/api?x=1|[object Request]"
    );
    let sent: Vec<(String, String, Option<String>)> = execution
        .effects
        .requests
        .iter()
        .map(|request| {
            (
                request.method.clone(),
                request.url.clone(),
                request.body.clone(),
            )
        })
        .collect();
    assert_eq!(
        sent,
        [
            (
                "POST".into(),
                "https://example.com/api?x=1".into(),
                Some("a=1".into())
            ),
            ("PUT".into(), "https://example.com/other".into(), None),
        ]
    );
}

/** Layout boxes the app supplies answer the geometry APIs; without them a page sees empty boxes. */
#[test]
fn geometry_comes_from_the_app_layout() {
    let mut environment = common::environment();
    environment.geometry = vec![
        mars_v9::NodeBox {
            node: 0,
            x: 10.0,
            y: 20.0,
            width: 300.0,
            height: 400.0,
            layout_position: None,
            ..mars_v9::NodeBox::default()
        },
        mars_v9::NodeBox {
            node: 3,
            x: 40.0,
            y: 60.0,
            width: 80.0,
            height: 30.0,
            layout_position: None,
            ..mars_v9::NodeBox::default()
        },
    ];
    environment.scroll_y = 15.0;
    let execution = load_with(
        "const go = document.getElementById('go'); const box = go.getBoundingClientRect();
         const hit = document.elementFromPoint(50, 50);
         const r = [box.x, box.y, box.width, box.height, box.bottom, go.offsetWidth, go.offsetTop, go.clientHeight,
           hit && hit.id, String(document.elementFromPoint(900, 900)), document.elementsFromPoint(50, 50).length].join('|');",
        &environment,
    );
    assert_eq!(failure(&execution), None);
    // elementsFromPoint lists the hit and every ancestor to the root, as Chrome does: the
    // button, div#main, body and html.
    assert_eq!(
        common::global(&execution, "r"),
        "40|45|80|30|75|80|60|30|go|null|4"
    );
    let bare =
        common::load("const r = document.getElementById('go').getBoundingClientRect().width;");
    assert_eq!(common::global(&bare, "r"), "0");
}

/** An upgraded custom element in the document records a definedness change for `:defined`. */
#[test]
fn upgrades_record_definedness() {
    let mut environment = common::environment();
    environment.dom_level = DomLevel::Extended;
    let execution = load_with(
        "const el = document.createElement('x-item'); document.getElementById('main').appendChild(el);
         customElements.define('x-item', class extends HTMLElement {});",
        &environment,
    );
    assert_eq!(failure(&execution), None);
    assert!(
        execution
            .script_changes
            .iter()
            .any(|change| matches!(change, mars_v9::document::ScriptChange::Defined { .. }))
    );
}

/** Measuring a connected node newer than the app's last layout is reported, so the app can lay
out and run the step again; nodes the layout saw, and detached nodes, are not. */
#[test]
fn measuring_a_node_newer_than_the_layout_is_reported() {
    let mut environment = common::environment();
    environment.geometry = vec![mars_v9::NodeBox {
        node: 0,
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        layout_position: None,
        ..mars_v9::NodeBox::default()
    }];
    environment.laid_out_before = Some(4);
    let stale = |source: &str, environment: &mars_v9::PageEnvironment| {
        load_with(source, environment).effects.stale_geometry
    };
    let inserted =
        "const d=document.createElement('div');document.body.appendChild(d);d.offsetWidth;";
    assert!(stale(inserted, &environment));
    assert!(!stale(
        "document.getElementById('first').getBoundingClientRect();",
        &environment
    ));
    assert!(!stale(
        "document.createElement('div').getBoundingClientRect();",
        &environment
    ));
    environment.laid_out_before = None;
    assert!(!stale(inserted, &environment));
}

/** `document.all` is the one undetectable object: falsy, typeof "undefined", loosely equal to
null, yet not `undefined` itself; its members still read the document. */
#[test]
fn document_all_is_undetectable() {
    let r = eval(
        "const all = document.all;
         const r = [typeof all, !all, all == null, all == undefined, all === undefined,
           undefined !== all, all.length > 0, all.item(0) === document.documentElement || all.item(0) !== null].join('|');",
        "r",
    );
    assert_eq!(r, "undefined|true|true|true|false|true|true|true");
}

/** Members live on their element's interface: a div has no `data`, `href` or `value`, which a
sanitizer tests with `in`, while anchors, inputs, objects and text nodes keep theirs. */
#[test]
fn members_belong_to_their_interfaces() {
    let r = eval(
        "const make = (tag) => document.createElement(tag);
         const div = make('div'), a = make('a'), input = make('input'), custom = make('x-thing');
         a.href = 'https://example.com/path'; input.value = 'typed';
         const r = ['data' in div, 'data' in custom, 'text' in custom, 'href' in div, 'value' in div,
           'href' in a, a.pathname, 'value' in input, input.value, 'data' in make('object'),
           'data' in document.createTextNode('x'), document.createTextNode('x').data,
           a instanceof HTMLAnchorElement, div instanceof HTMLAnchorElement, a instanceof HTMLElement,
           'hidden' in div, 'title' in custom, new Image().src === ''].join('|');",
        "r",
    );
    assert_eq!(
        r,
        "false|false|false|false|false|true|/path|true|typed|true|true|x|true|false|true|true|true|true"
    );
}

/** DOMParser parses an SVG string into an element a page can append (LinkedIn's icons), drops
the XML declaration, and gives an HTML document its head and body. */
#[test]
fn dom_parser_builds_appendable_trees() {
    let r = eval(
        "const svg = new DOMParser().parseFromString('<?xml version=\"1.0\"?><svg viewBox=\"0 0 24 24\"><path d=\"M0 0h24\"/></svg>', 'image/svg+xml').firstChild;
         const host = document.getElementById('first'); host.appendChild(svg);
         const doc = new DOMParser().parseFromString('<p class=x>hi</p>', 'text/html');
         const r = [svg.tagName, svg.firstChild.tagName, document.createElement('div').tagName, svg.getAttribute('viewBox'), svg.isConnected,
           host.querySelector('path') !== null, doc.body.querySelector('.x').textContent,
           doc.documentElement.tagName.toLowerCase(), doc.head.childNodes.length].join('|');",
        "r",
    );
    assert_eq!(r, "svg|path|DIV|0 0 24 24|true|true|hi|html|0");
}

/** URL's username and password are settable and `host` takes a port, as the URL Standard's
setters do; Airbnb's URL helpers assign `username` and threw without it. */
#[test]
fn url_credentials_and_host_port_are_settable() {
    let r = eval(
        "'use strict'; const u = new URL('https://a.example.com:8080/p');
         u.username = 'me'; u.password = 'pw'; u.host = 'c.example:9';
         const v = new URL('https://x.example/'); v.host = 'y.example';
         const r = [u.href, v.href].join('|');",
        "r",
    );
    assert_eq!(r, "https://me:pw@c.example:9/p|https://y.example/");
}

/** Path2D exists, so polyfills that extend its prototype run (Twitch's polyfill bundle). */
#[test]
fn path2d_exists_for_polyfills() {
    let r = eval(
        "Path2D.prototype.roundRect ??= function () {};
         const p = new Path2D(); p.moveTo(0, 0); p.lineTo(1, 1); p.closePath();
         const r = [typeof Path2D, p instanceof Path2D, typeof p.roundRect].join('|');",
        "r",
    );
    assert_eq!(r, "function|true|function");
}

/** Namespaced attribute methods work on the qualified name, as YouTube's player builds its SVG
icons: `setAttributeNS(xlink, 'xlink:href', v)` reads back through either form. */
#[test]
fn namespaced_attributes_round_trip() {
    let r = eval(
        "const x='http://www.w3.org/1999/xlink';\
         const u=document.createElementNS('http://www.w3.org/2000/svg','use');\
         u.setAttributeNS(x,'xlink:href','#shadow');\
         const a=[u.getAttribute('xlink:href'),u.getAttributeNS(x,'href'),u.hasAttributeNS(x,'href')];\
         u.removeAttributeNS(x,'href');\
         a.push(u.hasAttribute('xlink:href'),u.getAttributeNS(x,'href'));\
         let r=a.join(',');",
        "r",
    );
    assert_eq!(r, "#shadow,#shadow,true,false,");
}

/** DOM collections a page loops over are iterable, as in browsers: classList (its token set,
each once, with entries and forEach), children, query results, childNodes, attributes and an
element's inline style. YouTube spreads classList, and a TypeError there stopped its rendering. */
#[test]
fn dom_collections_are_iterable() {
    assert_eq!(
        eval(
            "const d=document.createElement('div');d.className='a b a';d.setAttribute('data-x','1');d.style.color='red';d.innerHTML='<p></p><p></p>';document.body.append(d);const out={};const t=(k,f)=>{try{out[k]=f()}catch(e){out[k]='ERR '+e.message}};t('classList',()=>[...d.classList].join());t('entries',()=>JSON.stringify([...d.classList.entries()]));t('forEach',()=>{const a=[];d.classList.forEach((v,i)=>a.push(i+v));return a.join()});t('children',()=>[...d.children].length);t('qsa',()=>[...d.querySelectorAll('p')].length);t('byTag',()=>[...d.getElementsByTagName('p')].length);t('childNodes',()=>[...d.childNodes].length);t('attributes',()=>[...d.attributes].map(a=>a.name).join());t('style',()=>[...d.style].join());t('forms',()=>[...document.forms].length);t('images',()=>[...document.images].length);let r=JSON.stringify(out);",
            "r"
        ),
        r#"{"classList":"a,b","entries":"[[0,\"a\"],[1,\"b\"]]","forEach":"0a,1b","children":2,"qsa":2,"byTag":2,"childNodes":2,"attributes":"class,data-x,style","style":"color","forms":0,"images":0}"#
    );
}

/** Element ids are not bounded like correlation ids: an empty id and one past 128 bytes parse,
set from script, and leave the page running, as YouTube's do; the long one is still found. */
#[test]
fn empty_and_long_element_ids_are_accepted() {
    let long = "x".repeat(300);
    let source = format!(
        "const d=document.createElement('div');document.body.append(d);d.id='';d.id='{long}';\
         const e=document.createElement('p');e.setAttribute('id','');document.body.append(e);\
         let r=[document.getElementById('{long}')===d,e.getAttribute('id'),d.id.length].join();"
    );
    assert_eq!(eval_extended(&source, "r"), "true,,300");
}

/** Run a script on the test page with the extended change log (scripts may set any attribute)
and return a global, failing on any script failure. */
fn eval_extended(source: &str, name: &str) -> String {
    let host = common::host();
    let mut environment = common::environment();
    environment.dom_level = mars_v9::DomLevel::Extended;
    let execution = mars_v9::load_page(mars_v9::PageInput {
        sources: &[source.to_string()],
        source_keys: &[],
        document: common::page(),
        storage: mars_v9::StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .expect("page loads");
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    common::global(&execution, name)
}
