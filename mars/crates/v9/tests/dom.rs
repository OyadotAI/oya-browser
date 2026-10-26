/** The DOM over the app's projection: queries, mutations in both change logs, and events. */
mod common;
use common::{console, eval, input, load, load_with};
use mars_v9::{
    DomLevel,
    document::{DomChange, InsertPosition, ScriptChange},
};

/** The extended-log environment. */
fn extended() -> mars_v9::PageEnvironment {
    mars_v9::PageEnvironment {
        dom_level: DomLevel::Extended,
        ..common::environment()
    }
}

/** Selectors: type, id, class, attribute operators, combinators and structural pseudo-classes. */
#[test]
fn selectors_match_over_the_projection() {
    let source = "const q=s=>[...document.querySelectorAll(s)].map(e=>e.id||e.className||e.tagName).join();\
        const r=[q('p'),q('#main > p'),q('div .note'),q('[data-kind=x]'),q('p:first-child'),q('p + p'),q('#first ~ button'),q(':not(p)'),q('p:nth-child(2)'),q('[class~=b]'),q('[id^=fi]')].join('|');";
    assert_eq!(
        eval(source, "r"),
        "first,note|first,note|note|note|first|note|go|HTML,HEAD,SCRIPT,BODY,main,go|note|main|first"
    );
}

/** NamedNodeMap's named getter finds an Attr by name, unenumerable, never over `length`. */
#[test]
fn attributes_are_readable_by_name() {
    let source = "const n=document.createElement('div');n.setAttribute('onfocusin','t');n.setAttribute('data-x','1');n.setAttribute('length','9');\
        const a=n.attributes;const r=[a['data-x'].value,a.onfocusin.expando,a['onfocusin'].name,a.length,Object.keys(a).join(':'),a.nope].join();";
    assert_eq!(eval(source, "r"), "1,,onfocusin,3,0:1:2,");
}

/** Lookups, traversal and element facts. */
#[test]
fn lookups_and_traversal() {
    let source = "const m=document.getElementById('main');const r=[m.tagName,m.children.length,m.firstElementChild.id,m.lastElementChild.id,m.parentNode===document.body,\
        document.body.parentNode===document.documentElement,document.getElementsByClassName('note').length,document.getElementsByTagName('p')[1].className,\
        m.classList.contains('b'),document.querySelector('.note').dataset.kind,m.matches('div.a'),document.getElementById('first').closest('#main')===m,\
        document.getElementById('nope')].join();";
    assert_eq!(
        eval(source, "r"),
        "DIV,3,first,go,true,true,1,note,true,x,true,true,"
    );
}

/** Created nodes stay local until inserted; insertion becomes one InsertHtml with fixed identities. */
#[test]
fn appended_elements_become_insert_html() {
    let source = "const d=document.createElement('section');d.id='made';d.className='x';d.appendChild(document.createTextNode('Hi & bye'));\
        const before=document.querySelectorAll('section').length;document.getElementById('main').appendChild(d);\
        const r=[before,document.querySelectorAll('section').length,d.parentNode.id,d.textContent,document.getElementById('made')===d].join();";
    let execution = load(source);
    assert_eq!(common::global(&execution, "r"), "0,1,main,Hi & bye,true");
    match &execution.changes[..] {
        [
            DomChange::InsertHtml {
                target,
                position,
                html,
                first_id,
            },
        ] => {
            assert_eq!(*target, Some(0));
            assert_eq!(*position, InsertPosition::BeforeEnd);
            assert_eq!(
                html,
                "<section class=\"x\" id=\"made\">Hi &amp; bye</section>"
            );
            assert_eq!(*first_id, 4);
        }
        other => panic!("unexpected changes {other:?}"),
    }
}

/** insertBefore of an existing node is one Move. */
#[test]
fn moving_existing_nodes_is_one_move() {
    let execution = load(
        "const m=document.getElementById('main');m.insertBefore(document.getElementById('go'),document.getElementById('first'));const r=m.firstElementChild.id;",
    );
    assert_eq!(common::global(&execution, "r"), "go");
    assert!(matches!(
        &execution.changes[..],
        [DomChange::Move {
            node: 3,
            target: Some(0),
            position: InsertPosition::AfterBegin
        }]
    ));
}

/** The legacy log refuses removal (the script fails alone) unless this run inserted the node. */
#[test]
fn legacy_log_refuses_removal_but_elides_probes() {
    let refused = load("document.getElementById('first').remove();");
    assert!(
        common::failure(&refused)
            .unwrap()
            .contains("extended change log")
    );
    let probe = load(
        "const d=document.createElement('div');document.body.appendChild(d);const inside=d.isConnected;d.remove();const r=[inside,d.isConnected].join();",
    );
    assert_eq!(common::global(&probe, "r"), "true,false");
    assert!(probe.changes.is_empty());
}

/** The extended log records removal, attributes, text and values in order. */
#[test]
fn extended_log_records_every_mutation() {
    let source = "const f=document.getElementById('first');f.setAttribute('title','t');f.classList.add('on');f.style.color='red';f.dataset.mode='m';\
        f.removeAttribute('title');f.textContent='new';document.querySelector('.note').remove();document.getElementById('go').hidden=true;\
        const r=[f.className,f.getAttribute('style'),f.dataset.mode,f.textContent,document.querySelectorAll('p').length].join('|');";
    let execution = load_with(source, &extended());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "on|color: red;|m|new|1");
    assert!(execution.changes.is_empty());
    let ops: Vec<String> = execution
        .script_changes
        .iter()
        .map(|change| match change {
            ScriptChange::Attribute { name, value, .. } => {
                format!("{name}={}", value.clone().unwrap_or_else(|| "-".into()))
            }
            ScriptChange::NodeText { text, .. } => format!("text:{text}"),
            ScriptChange::Remove { node } => format!("remove:{node}"),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        ops,
        [
            "title=t",
            "class=on",
            "style=color: red;",
            "data-mode=m",
            "title=-",
            "text:new",
            "remove:2",
            "hidden="
        ]
    );
}

/** Event handler attributes stay author-owned even in the extended log. */
#[test]
fn handler_attributes_are_refused() {
    let execution = load_with(
        "document.getElementById('first').setAttribute('onclick','alert(1)');",
        &extended(),
    );
    assert!(
        common::failure(&execution)
            .unwrap()
            .contains("not supported")
    );
}

/** innerHTML on detached elements builds real nodes with text; getters serialize. */
#[test]
fn inner_html_on_detached_elements() {
    let source = "const d=document.createElement('div');d.innerHTML='<p class=a>One<b>two</b></p><!--c--><input value=v>';\
        const r=[d.childNodes.length,d.firstChild.className,d.firstChild.textContent,d.querySelector('b').textContent,d.innerHTML].join('|');";
    assert_eq!(
        eval(source, "r"),
        "3|a|Onetwo|two|<p class=\"a\">One<b>two</b></p><!--c--><input value=\"v\">"
    );
}

/** document.implementation.createHTMLDocument gives a detached body (jQuery's support probe). */
#[test]
fn create_html_document_has_a_detached_body() {
    assert_eq!(
        eval(
            "const b=document.implementation.createHTMLDocument('').body;b.innerHTML='<form></form><form></form>';const r=b.childNodes.length;",
            "r"
        ),
        "2"
    );
}

/** Clicks bubble from the target to the document and window; element.click() dispatches too. */
#[test]
fn clicks_bubble_through_listeners() {
    let sources = vec!["const go=document.getElementById('go');go.addEventListener('click',e=>console.log('button',e.target.id,e.eventPhase));\
        document.getElementById('main').addEventListener('click',e=>console.log('main',e.currentTarget.id));\
        document.addEventListener('click',()=>console.log('document'),{capture:true});window.addEventListener('click',e=>{console.log('window');e.preventDefault()});".to_string()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    assert!(loaded.state.has_click_listener("page-1", 3));
    let clicked = mars_v9::click_page(
        input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert_eq!(
        console(&clicked),
        ["document", "button go 2", "main main", "window"]
    );
}

/** stopPropagation and once listeners. */
#[test]
fn propagation_control_and_once() {
    let source = "const go=document.getElementById('go');go.addEventListener('click',e=>{console.log('once')},{once:true});go.addEventListener('click',e=>e.stopPropagation());\
        document.body.addEventListener('click',()=>console.log('body'));go.click();go.click();";
    assert_eq!(console(&load(source)), ["once"]);
}

/** CustomEvent with detail through dispatchEvent. */
#[test]
fn custom_events_carry_detail() {
    let source = "document.addEventListener('hello',e=>console.log(e.detail.n,e.type,e instanceof Event));document.dispatchEvent(new CustomEvent('hello',{detail:{n:3}}));";
    assert_eq!(console(&load(source)), ["3 hello true"]);
}

/** An unprevented click on a link asks the app to navigate. */
#[test]
fn link_clicks_record_navigation() {
    let mut document = common::page();
    document
        .nodes
        .push(common::element(4, Some(0), "a", &[("href", "/docs")]));
    let sources = vec![
        "document.querySelector('a').addEventListener('click',()=>console.log('seen'));"
            .to_string(),
    ];
    let environment = common::environment();
    let host = common::host();
    let mut first = input(&sources, &host, &environment);
    first.document = document.clone();
    let loaded = mars_v9::load_page(first).unwrap();
    let mut second = input(&sources, &host, &environment);
    second.document = document;
    let clicked = mars_v9::click_page(second, loaded.state, 4).unwrap();
    assert_eq!(console(&clicked), ["seen"]);
    assert_eq!(
        clicked.effects.navigation.unwrap().url,
        "https://example.com/docs"
    );
}

/** Wrappers keep their identity and expando data across requests. */
#[test]
fn wrapper_identity_and_data_survive_requests() {
    let sources = vec!["const go=document.getElementById('go');go.myData={n:1};go.addEventListener('click',function(){this.myData.n++;console.log(this===go,this.myData.n)});".to_string()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(input(&sources, &host, &environment)).unwrap();
    let once = mars_v9::click_page(
        input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert_eq!(console(&once), ["true 2"]);
    let twice = mars_v9::click_page(
        input(&sources, &host, &environment),
        common::round_trip(once.state),
        3,
    )
    .unwrap();
    assert_eq!(console(&twice), ["true 3"]);
}

/** Each page script has a head script element; currentScript points at the running one. */
#[test]
fn page_scripts_have_script_elements() {
    let mut environment = common::environment();
    environment.scripts = vec!["https://cdn.example/a.js".into()];
    let execution = load_with(
        "const r=[document.currentScript.src,document.scripts.length,document.head.lastChild===document.currentScript].join();",
        &environment,
    );
    assert_eq!(
        common::global(&execution, "r"),
        "https://cdn.example/a.js,1,true"
    );
}

/** Style objects report only real CSS properties, so vendor probes behave like Chrome. */
#[test]
fn style_reports_chrome_properties() {
    assert_eq!(
        eval(
            "const s=document.body.style;const r=['color' in s,'MozAppearance' in s,'webkitTransform' in s,s.color].join();",
            "r"
        ),
        "true,false,true,"
    );
}

/** A `webkit`-prefixed style attribute names the `-webkit-` property, as CSSOM maps it, so
`style.webkitLineClamp = "none"` reaches `-webkit-line-clamp`. */
#[test]
fn webkit_style_attributes_name_dash_prefixed_properties() {
    assert_eq!(
        eval(
            "const s=document.body.style;s.webkitLineClamp='2';s.WebkitBoxOrient='vertical';\
             const r=[s.webkitLineClamp,s.getPropertyValue('-webkit-box-orient')].join();",
            "r"
        ),
        "2,vertical"
    );
}
