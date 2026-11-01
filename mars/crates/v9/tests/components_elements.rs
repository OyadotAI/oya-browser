/** Web Components in V9: custom elements, shadow roots and slots, templates, observers, traversal. */
use crate::common;
use common::{environment, global, host, page};
use mars_v9::{DomLevel, Execution, PageInput, SimpleFragmentParser, StorageState};

/** Load one script against the test page with the extended change log. */
fn run(source: &str) -> Execution {
    let host = host();
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    mars_v9::load_page(PageInput {
        sources: &[source.to_string()],
        source_keys: &[],
        module_keys: &[],
        document: page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .expect("page loads")
}

/** Run a script and return a global, failing on any script failure. */
fn eval(source: &str, name: &str) -> String {
    let execution = run(source);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    global(&execution, name)
}

#[test]
fn defined_elements_construct_and_run_their_lifecycle_callbacks() {
    let source = "const log=[];class XA extends HTMLElement{constructor(){super();log.push('ctor')}\
        static get observedAttributes(){return ['level']}\
        connectedCallback(){log.push('connected:'+this.isConnected)}disconnectedCallback(){log.push('disconnected')}\
        attributeChangedCallback(n,o,v){log.push(n+':'+o+'>'+v)}}\
        customElements.define('x-a',XA);const a=document.createElement('x-a');\
        a.setAttribute('level','1');a.setAttribute('other','2');document.body.appendChild(a);a.remove();\
        const r=[log.join(),a instanceof XA,a instanceof HTMLElement,a.localName,customElements.get('x-a')===XA].join('|');";
    assert_eq!(
        eval(source, "r"),
        "ctor,level:null>1,connected:true,disconnected|true|true|x-a|true"
    );
}

#[test]
fn elements_created_before_definition_upgrade_when_defined() {
    let source = "const d=document.getElementById('main');d.innerHTML='<x-b id=early level=3></x-b>';\
        const early=document.getElementById('early');const before=early.hello;let seen='';\
        customElements.whenDefined('x-b').then(c=>{seen=c.name});\
        class XB extends HTMLElement{static get observedAttributes(){return ['level']}\
        attributeChangedCallback(n,o,v){this.got=n+'='+v}connectedCallback(){this.c=1}get hello(){return 'hi'}}\
        customElements.define('x-b',XB);\
        const r=[String(before),early.hello,early.got,early.c,early instanceof XB].join('|');\
        const late=()=>seen;";
    let execution = run(source);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(global(&execution, "r"), "undefined|hi|level=3|1|true");
}

#[test]
fn the_html_element_constructor_refuses_undefined_classes() {
    let source = "let a='',b='';try{new HTMLElement()}catch(e){a=e.constructor.name}\
        class Nope extends HTMLElement{};try{new Nope()}catch(e){b=e.constructor.name}\
        let c='';try{customElements.define('nodash',class extends HTMLElement{})}catch(e){c=e.name}\
        const r=[a,b,c].join();";
    assert_eq!(eval(source, "r"), "TypeError,TypeError,SyntaxError");
}

#[test]
fn shadow_roots_hold_their_own_tree_out_of_light_dom_traversal() {
    let source = "const h=document.createElement('div');h.innerHTML='<span>light</span>';\
        const s=h.attachShadow({mode:'open'});s.innerHTML='<p class=q>shadow</p><slot></slot>';\
        document.body.appendChild(h);\
        const closed=document.createElement('x-c').attachShadow({mode:'closed'});\
        let again='';try{h.attachShadow({mode:'open'})}catch(e){again=e.name}\
        const r=[h.shadowRoot===s,s.host===h,s.mode,h.children.length,h.firstChild.nodeName,\
        s.querySelector('.q').textContent,document.querySelector('.q'),h.textContent,\
        s.firstChild.getRootNode()===s,s.parentNode,closed.host.shadowRoot,again,s instanceof ShadowRoot,s instanceof DocumentFragment].join('|');";
    assert_eq!(
        eval(source, "r"),
        "true|true|open|1|SPAN|shadow||light|true|||NotSupportedError|true|true"
    );
}

#[test]
fn slots_assign_light_children_by_name_and_signal_changes() {
    let source = "const h=document.createElement('div');h.innerHTML='<b slot=t>T</b><i>D</i>';\
        const s=h.attachShadow({mode:'open'});s.innerHTML='<slot name=t></slot><slot id=d></slot>';\
        document.body.appendChild(h);const d=s.getElementById('d');let changes=0;\
        d.addEventListener('slotchange',()=>changes++);\
        const before=d.assignedNodes().map(n=>n.nodeName).join();h.appendChild(document.createElement('u'));\
        const after=()=>d.assignedNodes().map(n=>n.nodeName).join()+'/'+changes;\
        const r=[before,s.querySelector('[name=t]').assignedElements()[0].textContent,h.firstChild.assignedSlot.name].join('|');\
        let later='';Promise.resolve().then(()=>{later=after()});";
    let execution = run(source);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(global(&execution, "r"), "I|T|t");
    assert_eq!(global(&execution, "later"), "I,U/1");
}

#[test]
fn template_content_is_a_fragment_that_clones_and_inserts() {
    let source = "const t=document.createElement('template');t.innerHTML='<li class=x>a</li><li>b</li>';\
        const c=document.importNode(t.content,true);const ul=document.createElement('ul');ul.appendChild(c);\
        document.getElementById('main').appendChild(ul);\
        const r=[t.childNodes.length,t.content.childNodes.length,t.content.nodeType,ul.children.length,\
        t.innerHTML,c.childNodes.length,t.content.querySelector('.x').textContent,t.cloneNode(true).content.childNodes.length].join('|');";
    assert_eq!(
        eval(source, "r"),
        "0|2|11|2|<li class=\"x\">a</li><li>b</li>|0|a|2"
    );
}

#[test]
fn mutation_observers_deliver_records_after_the_script() {
    let source = "const m=document.getElementById('main');const out=[];\
        const o=new MutationObserver((records,self)=>{for(const r of records)out.push(r.type+':'+(r.attributeName||'')+':'+r.oldValue+':'+r.addedNodes.length+':'+r.removedNodes.length);out.push(self===o)});\
        o.observe(m,{childList:true,attributes:true,attributeOldValue:true,subtree:true,characterData:true,characterDataOldValue:true});\
        m.setAttribute('class','z');const t=document.createTextNode('x');m.appendChild(t);t.data='y';\
        document.getElementById('first').setAttribute('title','q');m.removeChild(t);\
        const sync=out.length;let r='';Promise.resolve().then(()=>{r=sync+'|'+out.join()});";
    assert_eq!(
        eval(source, "r"),
        "0|attributes:class:a b:0:0,childList::null:1:0,characterData::x:0:0,attributes:title:null:0:0,childList::null:0:1,true"
    );
}

#[test]
fn tree_walkers_and_node_iterators_follow_filters() {
    let source = "const m=document.getElementById('main');\
        const w=document.createTreeWalker(m,NodeFilter.SHOW_ELEMENT,{acceptNode:n=>n.tagName==='P'?NodeFilter.FILTER_ACCEPT:NodeFilter.FILTER_SKIP});\
        const seen=[];while(w.nextNode())seen.push(w.currentNode.id||w.currentNode.className);\
        const back=w.previousNode()&&w.currentNode.id;w.currentNode=m;const first=w.firstChild().id;\
        const it=document.createNodeIterator(m,NodeFilter.SHOW_ALL);const all=[];let n;while((n=it.nextNode()))all.push(n.nodeName);\
        const r=[seen.join(),back,first,all.join(),NodeFilter.SHOW_ALL,typeof TreeWalker].join('|');";
    assert_eq!(
        eval(source, "r"),
        "first,note|first|first|DIV,P,P,BUTTON|4294967295|function"
    );
}

#[test]
fn clicks_in_a_shadow_tree_reach_the_host_retargeted() {
    let source = "const h=document.createElement('x-host');const s=h.attachShadow({mode:'open'});\
        s.innerHTML='<button id=inner>go</button>';document.body.appendChild(h);const seen=[];\
        h.addEventListener('click',e=>seen.push('host:'+e.target.localName+':'+e.composedPath().length));\
        document.addEventListener('click',e=>seen.push('doc:'+e.target.localName));\
        s.addEventListener('click',e=>seen.push('root:'+e.target.id));\
        s.getElementById('inner').dispatchEvent(new Event('click',{bubbles:true,composed:true}));\
        s.getElementById('inner').dispatchEvent(new CustomEvent('local',{bubbles:true}));\
        h.addEventListener('local',()=>seen.push('leaked'));\
        s.getElementById('inner').dispatchEvent(new CustomEvent('local',{bubbles:true}));\
        const r=seen.join();";
    assert_eq!(eval(source, "r"), "root:inner,host:x-host:7,doc:x-host");
}

#[test]
fn canvas_contexts_v9_cannot_draw_are_null() {
    assert_eq!(
        eval(
            "const r=String(document.createElement('canvas').getContext('2d'));",
            "r"
        ),
        "null"
    );
}

#[test]
fn projected_hosts_keep_their_shadow_root_through_the_change_log() {
    let source = "const m=document.getElementById('main');const s=m.attachShadow({mode:'open'});\
        const b=document.createElement('b');b.textContent='in';s.append(b,document.createElement('slot'));const keep=m.shadowRoot===s;\
        document.body.appendChild(document.createElement('section')).appendChild(m);\
        const r=[keep,m.shadowRoot.firstChild.textContent,m.children.length,m.isConnected].join('|');";
    let execution = run(source);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(global(&execution, "r"), "true|in|3|true");
    let mut document = page();
    let mut texts = environment().node_text;
    mars_v9::replay(&mut document, &execution, &SimpleFragmentParser, &mut texts).unwrap();
    let main = document
        .nodes
        .iter()
        .find(|node| node.html_id == "main")
        .unwrap()
        .local_id;
    let root = document
        .nodes
        .iter()
        .find(|node| node.tag == "#shadow-root")
        .expect("the projection holds the shadow root");
    assert_eq!(root.parent, Some(main));
    assert!(
        document
            .nodes
            .iter()
            .any(|node| node.tag == "b" && node.parent == Some(root.local_id))
    );
}

/** A definition made after hundreds of matching elements exist upgrades each one once, in tree
order, and leaves elements of other names alone. */
#[test]
fn elements_defined_after_many_nodes_upgrade_in_tree_order() {
    let source = "const d=document.getElementById('main');let h='<x-u id=u1></x-u>';\
        for(let i=0;i<200;i++)h+='<x-t id=t'+i+'></x-t>';d.innerHTML=h;\
        const order=[];class XT extends HTMLElement{constructor(){super();order.push(this.id)}}\
        customElements.define('x-t',XT);\
        const r=[order.length,order[0],order[199],order.join(',')===Array.from({length:200},(_,i)=>'t'+i).join(','),\
        document.getElementById('u1') instanceof XT].join('|');";
    assert_eq!(eval(source, "r"), "200|t0|t199|true|false");
}

/** An element removed before its name is defined stays out of the upgrade until it connects again. */
#[test]
fn elements_removed_before_definition_upgrade_only_when_connected_again() {
    let source = "const d=document.getElementById('main');d.innerHTML='<x-rm id=rm></x-rm>';\
        const gone=document.getElementById('rm');gone.remove();let runs=0;\
        class XR extends HTMLElement{constructor(){super();runs++}}\
        customElements.define('x-rm',XR);\
        const before=[runs,gone instanceof XR].join(':');\
        d.appendChild(gone);\
        const r=[before,runs,gone instanceof XR].join('|');";
    assert_eq!(eval(source, "r"), "0:false|1|true");
}

/** An element removed and re-inserted before its definition upgrades exactly once, in place. */
#[test]
fn elements_reinserted_before_definition_upgrade_once() {
    let source = "const d=document.getElementById('main');d.innerHTML='<x-ri id=a></x-ri><x-ri id=b></x-ri>';\
        const a=document.getElementById('a');const b=document.getElementById('b');\
        d.insertBefore(a,null);d.insertBefore(b,a);d.removeChild(b);d.appendChild(b);\
        let runs=[];class XI extends HTMLElement{constructor(){super();runs.push(this.id)}}\
        customElements.define('x-ri',XI);\
        const r=[runs.join(','),d.lastElementChild===b,a instanceof XI].join('|');";
    assert_eq!(eval(source, "r"), "a,b|true|true");
}

/** Elements in an open shadow tree of a connected host upgrade when their definition arrives. */
#[test]
fn shadow_tree_elements_upgrade_when_their_name_is_defined() {
    let source = "const h=document.createElement('div');document.body.appendChild(h);\
        const s=h.attachShadow({mode:'open'});s.innerHTML='<x-sh id=in></x-sh>';\
        const detachedHost=document.createElement('div');const ds=detachedHost.attachShadow({mode:'open'});\
        ds.innerHTML='<x-sh id=off></x-sh>';\
        class XS extends HTMLElement{}\
        customElements.define('x-sh',XS);\
        const r=[s.getElementById('in') instanceof XS,ds.getElementById('off') instanceof XS].join('|');";
    assert_eq!(eval(source, "r"), "true|false");
}

/** An element created but never inserted is not upgraded by a definition; it upgrades on connection. */
#[test]
fn created_elements_upgrade_only_once_connected() {
    let source = "let runs=0;const made=document.createElement('x-cr');\
        class XC extends HTMLElement{constructor(){super();runs++}}\
        customElements.define('x-cr',XC);\
        const before=[runs,made instanceof XC].join(':');\
        document.getElementById('main').appendChild(made);\
        const r=[before,runs,made instanceof XC].join('|');";
    assert_eq!(eval(source, "r"), "0:false|1|true");
}

/** Elements inserted after an earlier definition are found by a later definition of their own name. */
#[test]
fn later_definitions_find_elements_inserted_after_earlier_ones() {
    let source = "const d=document.getElementById('main');\
        class XA extends HTMLElement{}customElements.define('x-early',XA);\
        d.innerHTML='<x-late id=l1></x-late><x-late id=l2></x-late><x-early></x-early>';\
        const order=[];class XL extends HTMLElement{constructor(){super();order.push(this.id)}}\
        customElements.define('x-late',XL);\
        const r=[order.join(','),document.getElementById('l2') instanceof XL,\
        document.querySelector('x-early') instanceof XA].join('|');";
    assert_eq!(eval(source, "r"), "l1,l2|true|true");
}

/** A customized built-in whose `is` is written after insertion upgrades when its definition arrives. */
#[test]
fn is_written_after_insertion_upgrades_at_definition() {
    let source = "const d=document.getElementById('main');d.innerHTML='<button id=bt></button>';\
        const bt=document.getElementById('bt');bt.setAttribute('is','x-btn');\
        class XB2 extends HTMLElement{}\
        customElements.define('x-btn',XB2,{extends:'button'});\
        const r=[bt instanceof XB2,document.createElement('button') instanceof XB2].join('|');";
    assert_eq!(eval(source, "r"), "true|false");
}

/** A definition whose name matches no element upgrades nothing and leaves other elements alone. */
#[test]
fn definitions_without_matching_elements_leave_others_alone() {
    let source = "const d=document.getElementById('main');d.innerHTML='<x-keep id=k></x-keep>';\
        const keep=document.getElementById('k');\
        class XN extends HTMLElement{}customElements.define('x-none',XN);\
        const r=[keep instanceof XN,keep.localName].join('|');";
    assert_eq!(eval(source, "r"), "false|x-keep");
}
