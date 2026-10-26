/** Inert data blocks stay where the source puts them, as GitHub's React partials need: each
`<react-partial>` reads its props from its own child `<script type=application/json>`, and a
head copy of that block is not found by a lookup on the partial ("No embedded data provided"). */
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

/** A data block inside an element is that element's child, before the siblings parsed after
it, and its JSON reads through textContent. */
#[test]
fn react_partial_finds_its_embedded_data_child() {
    let (_fixture, _runtime, page) = load(
        "<div><react-partial partial-name=header>\
         <script type=\"application/json\" data-target=\"react-partial.embeddedData\">{\"props\":{\"n\":3}}</script>\
         <div data-target=\"react-partial.reactRoot\"></div></react-partial></div>\
         <p id=out>initial</p><script>\
         const p=document.querySelector('react-partial');\
         const s=p.querySelector('script[data-target=\"react-partial.embeddedData\"]');\
         const root=p.querySelector('[data-target=\"react-partial.reactRoot\"]');\
         document.getElementById('out').textContent='n='+(s?JSON.parse(s.textContent).props.n:'none')+\
         ' parent='+(s!==null&&s.parentNode===p)+' first='+(p.firstElementChild===s)+\
         ' next='+(s!==null&&s.nextElementSibling===root)+' head='+document.head.contains(s);\
         </script>",
    );
    let text = texts(&page);
    assert!(
        text.contains("n=3 parent=true first=true next=true head=false"),
        "{page}"
    );
    assert!(!text.contains("\"props\""), "data never paints: {page}");
}

/** A data block between siblings keeps its place among them, at the top level of the body
too, while head metadata stays in the head. */
#[test]
fn data_blocks_keep_their_sibling_places() {
    let (_fixture, _runtime, page) = load(
        "<html><head><meta name=locale content=en></head><body>\
         <div id=box><span id=a></span><script type=application/json id=d>1</script><span id=b></span></div>\
         <p id=out>initial</p><script type=application/json id=top>2</script><i id=after></i><script>\
         const d=document.getElementById('d'),t=document.getElementById('top');\
         document.getElementById('out').textContent='prev='+d.previousElementSibling.id+\
         ' next='+d.nextElementSibling.id+' top='+(t.parentNode===document.body)+\
         ' topnext='+t.nextElementSibling.id+' meta='+(document.querySelector('meta').parentNode===document.head);\
         </script></body></html>",
    );
    assert!(
        texts(&page).contains("prev=a next=b top=true topnext=after meta=true"),
        "{page}"
    );
}

/** A removed data block leaves its parent: a second lookup finds nothing. */
#[test]
fn removed_data_block_leaves_its_parent() {
    let (_fixture, _runtime, page) = load(
        "<section id=s><script type=application/json id=d>{}</script></section>\
         <p id=out>initial</p><script>\
         const s=document.getElementById('s');s.querySelector('script').remove();\
         document.getElementById('out').textContent='left='+(s.querySelector('script')===null)+\
         ' count='+s.children.length;\
         </script>",
    );
    assert!(texts(&page).contains("left=true count=0"), "{page}");
}
