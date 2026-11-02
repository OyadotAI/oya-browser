/** Static NodeList reads in V9: index and `length` reads answer from the snapshot a query made,
unaffected by later DOM changes, and an out-of-range index is undefined. */
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
fn static_list_index_and_length_answer_from_the_snapshot() {
    let source = "const p=document.createElement('div');document.body.append(p);\
        for(let i=0;i<3;i++){const s=document.createElement('span');s.className='snap';p.append(s);}\
        const list=p.querySelectorAll('.snap');const before=list.length;\
        p.append(document.createElement('span'));\
        const r=[before,list.length,list[2]===p.lastElementChild.previousElementSibling,list[3]===undefined].join('|');";
    assert_eq!(eval(source, "r"), "3|3|true|true");
}

#[test]
fn static_list_non_index_keys_other_than_length_are_undefined() {
    let source = "const list=document.querySelectorAll('html');const r=[list.nope===undefined,list[-1]===undefined].join('|');";
    assert_eq!(eval(source, "r"), "true|true");
}

/** NodeList's forEach is the generic Array one, so a Proxy of a NodeList can run it, as Chrome allows;
its callback sees each node, its index and the receiver. */
#[test]
fn node_list_for_each_runs_on_a_proxy_of_a_node_list() {
    let source = "const p=new Proxy(document.querySelectorAll('p'),{});let n=0;NodeList.prototype.forEach.call(p,(e,i,list)=>{n+=1+i;if(list!==p)n=-1000});const r=String(n);";
    assert_eq!(common::eval(source, "r"), "3");
}
