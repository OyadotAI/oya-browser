/** Live HTMLCollection behaviour in V9: getElementsByTagName, getElementsByTagNameNS and
getElementsByClassName follow the tree as it changes, while querySelectorAll stays a snapshot. The
collections answer from a cache keyed on the tree generation, so each test reads after a mutation. */
use crate::common;
use common::{environment, global, host, page};
use mars_v9::{DomLevel, PageInput, StorageState};

/** Run a script against the test page and return a global, failing on any script failure. */
fn eval(source: &str, name: &str) -> String {
    let host = host();
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    let execution = mars_v9::load_page(PageInput {
        sources: &[source.to_string()],
        source_keys: &[],
        module_keys: &[],
        document: page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .expect("page loads");
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    global(&execution, name)
}

#[test]
fn tag_collection_sees_elements_appended_after_it_was_taken() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        const list=p.getElementsByTagName('span');const before=list.length;\
        p.append(document.createElement('span'));\
        const r=[before,list.length].join('|');";
    assert_eq!(eval(source, "r"), "0|1");
}

#[test]
fn tag_collection_drops_elements_removed_from_the_tree() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        for(let i=0;i<3;i++)p.append(document.createElement('i'));\
        const list=p.getElementsByTagName('i');const first=list[0];\
        p.removeChild(first);\
        const r=[list.length,list[0]===first,list.item(0)!==first].join('|');";
    assert_eq!(eval(source, "r"), "2|false|true");
}

#[test]
fn tag_collection_repeats_the_same_answer_between_mutations() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        p.append(document.createElement('b'));\
        const list=p.getElementsByTagName('b');\
        const r=[list.length,list.length,list[0]===list[0],list[0]===p.firstChild].join('|');";
    assert_eq!(eval(source, "r"), "1|1|true|true");
}

#[test]
fn class_collection_follows_class_writes_on_its_elements() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        const a=document.createElement('span');p.append(a);\
        const list=p.getElementsByClassName('hit');const before=list.length;\
        a.className='hit';\
        const after=list.length;\
        a.classList.remove('hit');\
        const r=[before,after,list.length].join('|');";
    assert_eq!(eval(source, "r"), "0|1|0");
}

#[test]
fn empty_class_name_matches_no_element() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        p.className='x';\
        const r=p.getElementsByClassName('').length;";
    assert_eq!(eval(source, "r"), "0");
}

#[test]
fn class_collection_matches_every_listed_class_in_any_order() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        const a=document.createElement('span');a.className='x y';p.append(a);\
        const b=document.createElement('span');b.className='y';p.append(b);\
        const r=p.getElementsByClassName('y x').length;";
    assert_eq!(eval(source, "r"), "1");
}

#[test]
fn namespace_collection_matches_html_and_wildcard_namespaces_only() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        p.append(document.createElement('em'));\
        const r=[p.getElementsByTagNameNS('http://www.w3.org/1999/xhtml','em').length,\
        p.getElementsByTagNameNS('*','em').length,\
        p.getElementsByTagNameNS('urn:other','em').length].join('|');";
    assert_eq!(eval(source, "r"), "1|1|0");
}

#[test]
fn namespace_collection_with_null_namespace_matches_nothing() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        p.append(document.createElement('em'));\
        const r=[p.getElementsByTagNameNS(null,'em').length,\
        p.getElementsByTagNameNS('','em').length].join('|');";
    assert_eq!(eval(source, "r"), "0|0");
}

#[test]
fn namespace_collection_on_document_is_live() {
    let source = "const list=document.getElementsByTagNameNS('*','p');\
        const before=list.length;\
        const p=document.createElement('p');document.body.append(p);\
        const r=String(list.length-before);";
    assert_eq!(eval(source, "r"), "1");
}

#[test]
fn query_selector_all_stays_a_snapshot_while_tag_collections_are_live() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        p.append(document.createElement('u'));\
        const snap=p.querySelectorAll('u');const live=p.getElementsByTagName('u');\
        p.append(document.createElement('u'));\
        const r=[snap.length,live.length].join('|');";
    assert_eq!(eval(source, "r"), "1|2");
}

#[test]
fn html_collection_is_an_html_collection_and_not_a_node_list() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        const list=p.getElementsByTagName('i');\
        const r=[list instanceof HTMLCollection,list instanceof NodeList,\
        Object.prototype.toString.call(list)].join('|');";
    assert_eq!(eval(source, "r"), "true|false|[object HTMLCollection]");
}

#[test]
fn html_collection_has_no_for_each_entries_keys_or_values_methods() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        const list=p.getElementsByTagName('i');\
        const r=['forEach','entries','keys','values'].map(k=>k in list).join('|');";
    assert_eq!(eval(source, "r"), "false|false|false|false");
}

#[test]
fn html_collection_iterates_its_current_elements_with_for_of() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        for(let i=0;i<2;i++){const s=document.createElement('i');s.id='n'+i;p.append(s);}\
        const list=p.getElementsByTagName('i');const seen=[];\
        for(const el of list)seen.push(el.id);\
        const r=seen.join(',');";
    assert_eq!(eval(source, "r"), "n0,n1");
}

#[test]
fn collection_on_a_detached_root_follows_that_subtree() {
    let source = "const root=document.createElement('section');\
        const list=root.getElementsByTagName('a');\
        const link=document.createElement('a');root.append(link);\
        const r=[list.length,list[0]===link].join('|');";
    assert_eq!(eval(source, "r"), "1|true");
}

#[test]
fn collection_index_past_the_end_is_undefined_after_a_removal() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        p.append(document.createElement('q'));\
        const list=p.getElementsByTagName('q');\
        p.removeChild(p.firstChild);\
        const r=[list.length,list[0]===undefined,list.item(0)===null].join('|');";
    assert_eq!(eval(source, "r"), "0|true|true");
}
