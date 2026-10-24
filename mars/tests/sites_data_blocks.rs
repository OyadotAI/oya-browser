/** Page data scripts read the way LinkedIn and Google read it: configuration JSON inside a
`<code>` comment, head metadata, a stylesheet link in the body and inert JSON data blocks, all
found through the real owned services while none of it paints. */
#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use support::{Fixture, agent_call, desktop, html};

/** Navigate one fixture page whose source is `source`. */
fn load(source: &str) -> (Fixture, mars::runtime::Runtime, Value) {
    let page: &'static str = Box::leak(source.to_string().into_boxed_str());
    let fixture = Fixture::routes(vec![("/", html("", page))]);
    let mut runtime = desktop(&fixture.url);
    let page = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    (fixture, runtime, page)
}

/** Text of the inspected nodes, joined, so assertions read what a person sees. */
fn texts(page: &Value) -> String {
    page["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/** LinkedIn's embedJson reader: the comment child of a `<code>` element parses as JSON. (LinkedIn
compares with `Node.COMMENT_NODE`, which V9 defines only on Node.prototype so far.) */
#[test]
fn code_comment_json_is_a_comment_child() {
    let (_fixture, _runtime, page) = load(
        "<div><code id=flag style=\"display:none\"><!--{\"on\":true}--></code>\
         <p id=out>initial</p></div><script>\
         const code=document.getElementById('flag');let v='missing';\
         for(const n of code.childNodes){if(n.nodeType===8){v=JSON.parse(n.nodeValue).on}}\
         document.getElementById('out').textContent='flag='+v+' text='+code.textContent.length;\
         code.parentNode.removeChild(code);</script>",
    );
    let text = texts(&page);
    assert!(text.contains("flag=true text=0"), "{page}");
    assert!(
        !text.contains("\"on\""),
        "comment data never paints: {page}"
    );
}

/** Head metadata answers selector and id lookups (LinkedIn's meta locale). */
#[test]
fn head_metadata_is_found_by_scripts() {
    let (_fixture, _runtime, page) = load(
        "<html><head><title>T</title><meta name=locale content=en_US>\
         <meta id=config data-page-instance=p1></head><body><p id=out>initial</p><script>\
         document.getElementById('out').textContent=document.querySelector('meta[name=\"locale\"]').content+\
         ' '+document.getElementById('config').dataset.pageInstance+' '+document.head.querySelector('title').textContent;\
         </script></body></html>",
    );
    assert!(texts(&page).contains("en_US p1 T"), "{page}");
}

/** Google's late stylesheet swap: a body `<link>` is found and its media and id change. */
#[test]
fn body_link_is_found_and_writable() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><link id=ogb_ss rel=stylesheet media=print href=/x.css><script>\
         const l=document.getElementById('ogb_ss');l.onload=()=>{l.media='all'};l.removeAttribute('id');\
         document.getElementById('out').textContent='media='+l.getAttribute('media')+' gone='+(document.getElementById('ogb_ss')===null);\
         </script>",
    );
    assert!(texts(&page).contains("media=print gone=true"), "{page}");
}

/** A JSON data block never runs, and scripts read it as text. */
#[test]
fn json_data_block_is_inert_and_readable() {
    let (_fixture, _runtime, page) = load(
        "<p id=out>initial</p><script type=application/json id=data>{\"n\":2}</script>\
         <script type=text/x-template id=tpl>window.ran=1</script><script>\
         const d=JSON.parse(document.getElementById('data').textContent).n;\
         document.getElementById('out').textContent='n='+d+' ran='+(window.ran===undefined?'no':'yes')+\
         ' tpl='+document.getElementById('tpl').textContent;</script>",
    );
    let text = texts(&page);
    assert!(text.contains("n=2 ran=no tpl=window.ran=1"), "{page}");
}
