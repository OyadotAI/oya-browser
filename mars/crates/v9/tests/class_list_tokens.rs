/** DOMTokenList in V9 over the token set: order, duplicates, toggle and replace follow the DOM rules,
and a write made outside classList is seen by the next call, so the kept set never goes stale. */
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

/** add appends tokens it does not hold, and leaves the order of the ones it does. */
#[test]
fn add_appends_new_tokens_and_keeps_the_order_of_the_rest() {
    let source = "const d=document.createElement('div');d.className='a b';d.classList.add('c','a');\
        const r=d.className;";
    assert_eq!(eval(source, "r"), "a b c");
}

/** A class attribute repeating a token counts it once, as the DOM's ordered set does. */
#[test]
fn a_repeated_token_in_the_attribute_counts_once() {
    let source = "const d=document.createElement('div');d.className='a a b';\
        const n=d.classList.length;d.classList.add('x');const r=[n,d.className].join('|');";
    assert_eq!(eval(source, "r"), "2|a b x");
}

/** remove drops the named tokens and keeps the order of what is left. */
#[test]
fn remove_keeps_the_order_of_the_remaining_tokens() {
    let source = "const d=document.createElement('div');d.className='a b c d';\
        d.classList.remove('b','d','missing');const r=d.className;";
    assert_eq!(eval(source, "r"), "a c");
}

/** toggle flips a token, and an explicit force decides without flipping. */
#[test]
fn toggle_flips_and_honors_an_explicit_force() {
    let source = "const d=document.createElement('div');\
        const r=[d.classList.toggle('x'),d.classList.toggle('x'),d.classList.toggle('y',true),\
        d.classList.toggle('y',true),d.classList.toggle('y',false),d.className==='' ].join('|');";
    assert_eq!(eval(source, "r"), "true|false|true|true|false|true");
}

/** replace swaps a token where it sits, and reports false when the old token is absent. */
#[test]
fn replace_swaps_in_place_and_reports_a_missing_token() {
    let source = "const d=document.createElement('div');d.className='a b c';\
        const found=d.classList.replace('b','z');const missing=d.classList.replace('q','r');\
        const r=[found,missing,d.className].join('|');";
    assert_eq!(eval(source, "r"), "true|false|a z c");
}

/** replace leaves one copy of the new token, where the old one sat. */
#[test]
fn replace_drops_other_copies_of_the_new_token() {
    let source = "const d=document.createElement('div');d.className='a b c';\
        d.classList.replace('a','c');const r=d.className;";
    assert_eq!(eval(source, "r"), "c b");
}

/** item, contains and length read the set: out of range and negative indexes give null. */
#[test]
fn item_contains_and_length_read_the_set() {
    let source = "const d=document.createElement('div');d.className='a b';\
        const r=[d.classList.item(1),d.classList.item(2)===null,d.classList.item(-1)===null,\
        d.classList.contains('a'),d.classList.contains('z'),d.classList.length].join('|');";
    assert_eq!(eval(source, "r"), "b|true|true|true|false|2");
}

/** iteration yields the tokens in the attribute's order. */
#[test]
fn iteration_yields_tokens_in_order() {
    let source = "const d=document.createElement('div');d.className='q w e';\
        const r=[...d.classList].join(',');";
    assert_eq!(eval(source, "r"), "q,w,e");
}

/** A class written by a script outside classList replaces the kept set: the next call reads it. */
#[test]
fn a_write_outside_class_list_is_seen_by_the_next_call() {
    let source = "const d=document.createElement('div');d.classList.add('a');\
        d.className='x y';const stale=d.classList.contains('a');d.classList.add('z');\
        const r=[stale,d.className].join('|');";
    assert_eq!(eval(source, "r"), "false|x y z");
}

/** Many adds stay cheap: a set of two thousand tokens holds two thousand, in the order added. */
#[test]
fn many_adds_keep_every_token_in_order() {
    let source = "const d=document.createElement('div');\
        for(let i=0;i<2000;i++){d.classList.add('t'+i);}\
        const r=[d.classList.length,d.classList.item(1999)].join('|');";
    assert_eq!(eval(source, "r"), "2000|t1999");
}

/** A token list over another attribute, such as sandbox, writes that attribute. */
#[test]
fn a_token_list_over_another_attribute_writes_that_attribute() {
    let source = "const f=document.createElement('iframe');f.sandbox.add('allow-scripts');\
        f.sandbox.add('allow-forms');const r=f.getAttribute('sandbox');";
    assert_eq!(eval(source, "r"), "allow-scripts allow-forms");
}

/** supports is not implemented for class lists, so it throws a TypeError rather than answering. */
#[test]
fn supports_throws_for_a_class_list() {
    let source = "const d=document.createElement('div');let r='none';\
        try{d.classList.supports('x');}catch(e){r=e.name;}";
    assert_eq!(eval(source, "r"), "TypeError");
}
