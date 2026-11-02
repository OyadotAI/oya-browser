/** DOM sequences from real pages that cross requests (python.org's Modernizr then jQuery). */
use crate::common;
use common::{environment, host, page, round_trip};
use mars_v9::{DomLevel, PageEvent, PageInput, SimpleFragmentParser, StorageState};

/** A node a script inserted and removed keeps its id, so a later request's insertion gets a fresh
one and its wrapper finds the right node. */
#[test]
fn removed_nodes_keep_their_ids_across_requests() {
    let source = "var l=document.createElement('div');l.innerHTML='<b>q</b>';document.body.appendChild(l);l.parentNode.removeChild(l);\
        document.addEventListener('DOMContentLoaded',function(){var c=document.createElement('div');document.body.insertBefore(c,document.body.firstChild);\
        var d=document.createElement('span');c.appendChild(d);console.log(c.parentNode.nodeName+' '+d.parentNode.nodeName+' '+l.firstChild.nodeName)});"
        .to_string();
    let sources = [source];
    let host = host();
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    let input = |document| PageInput {
        sources: &sources,
        source_keys: &[],
        module_keys: &[],
        document,
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    };
    let loaded = mars_v9::load_page(input(page())).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    let mut document = page();
    let mut texts = environment.node_text.clone();
    mars_v9::replay(&mut document, &loaded, &SimpleFragmentParser, &mut texts).unwrap();
    let ready = mars_v9::page_event(
        input(document),
        round_trip(loaded.state),
        PageEvent::DomContentLoaded,
    )
    .unwrap();
    assert_eq!(common::console(&ready), ["BODY DIV B"]);
}

/** An inserted `<style>` the document parser keeps as a sheet stays a detached node for scripts. */
#[test]
fn inserted_styles_the_parser_drops_stay_detached() {
    let source = "var l=document.createElement('div');l.innerHTML='&#173;<style id=s>#m{width:1px}</style>';document.body.appendChild(l);\
        const r=[l.isConnected,l.parentNode.nodeName].join();"
        .to_string();
    let execution = common::load(&source);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(common::global(&execution, "r"), "true,BODY");
}
