/** DOM tree semantics ported from Blink: pre-insertion validity, the ChildNode and ParentNode
steps, CharacterData, document.createEvent and the event dispatch flags. */
mod common;
use common::eval;

/** Each rejected insertion throws the DOMException the DOM standard names. */
#[test]
fn pre_insertion_validity_throws_dom_exceptions() {
    let source = "const n=(f)=>{try{f();return 'ok'}catch(e){return e.name+(e instanceof DOMException?'':'!')}};\
        const a=document.createElement('a'),b=document.createElement('b'),t=document.createTextNode('x');a.appendChild(b);\
        const r=[n(()=>b.appendChild(a)),n(()=>t.appendChild(b)),n(()=>a.insertBefore(t,document.createElement('i'))),\
        n(()=>a.removeChild(t)),n(()=>a.replaceChild(t,document.body)),n(()=>a.appendChild(document)),\
        n(()=>a.replaceChild(b,b)),a.firstChild===b].join();";
    assert_eq!(
        eval(source, "r"),
        "HierarchyRequestError,HierarchyRequestError,NotFoundError,NotFoundError,NotFoundError,HierarchyRequestError,ok,true"
    );
}

/** replaceChild puts the new node where the old one was, also when they are siblings. */
#[test]
fn replace_child_keeps_position() {
    let source = "const p=document.createElement('p');p.innerHTML='<i></i><b></b><u></u>';\
        p.replaceChild(p.lastChild,p.firstChild);const r=[...p.childNodes].map(c=>c.tagName).join();";
    assert_eq!(eval(source, "r"), "U,B");
}

/** before, after and replaceWith find a viable sibling that is not among the moved nodes. */
#[test]
fn child_node_steps_use_viable_siblings() {
    let source = "const p=document.createElement('p');p.innerHTML='<i></i><b></b><u></u>';const [i,b,u]=[...p.children];\
        b.before(b,u);const one=[...p.childNodes].map(c=>c.tagName).join('');\
        i.after(u,'t',i);const two=[...p.childNodes].map(c=>c.tagName||c.data).join('');\
        b.replaceWith(b);const three=[...p.childNodes].map(c=>c.tagName||c.data).join('');\
        const r=[one,two,three].join('|');";
    assert_eq!(eval(source, "r"), "IBU|UtIB|UtIB");
}

/** CharacterData edits count UTF-16 units and reject offsets past the end. */
#[test]
fn character_data_methods() {
    let source = "const t=document.createTextNode('hello');t.appendData('!');t.insertData(0,'>');t.deleteData(1,1);\
        t.replaceData(1,2,'EL');const sub=t.substringData(1,100);let e='';try{t.substringData(99,1)}catch(x){e=x.name}\
        const p=document.createElement('p');p.appendChild(t);const s=t.splitText(3);\
        const c=document.createComment('c');p.appendChild(c);c.before('x');c.remove();\
        const r=[t.data,s.data,sub,e,p.childNodes.length,p.textContent].join();";
    assert_eq!(eval(source, "r"), ">EL,lo!,ELlo!,IndexSizeError,3,>ELlo!x");
}

/** createEvent knows the standard's interface names, and its event must be initialized first. */
#[test]
fn create_event_table_and_initialized_flag() {
    let source = "const n=(f)=>{try{f();return 'ok'}catch(e){return e.name}};\
        const e=document.createEvent('MouseEvents');const before=n(()=>document.dispatchEvent(e));\
        e.initEvent('x',true,true);const after=n(()=>document.dispatchEvent(e));\
        const r=[e instanceof MouseEvent,before,after,n(()=>document.createEvent('Nope')),n(()=>new Event())].join();";
    assert_eq!(
        eval(source, "r"),
        "true,InvalidStateError,ok,NotSupportedError,TypeError"
    );
}

/** A stop flag set before dispatch keeps every listener from running, and dispatch clears
it afterwards; stopPropagation in a capture listener at the target skips the bubble ones. */
#[test]
fn dispatch_honors_and_clears_stop_flags() {
    let source = "const d=document.createElement('div');let log=[];\
        d.addEventListener('x',()=>log.push('bubble'));d.addEventListener('x',e=>{log.push('capture');e.stopPropagation()},true);\
        const e=new Event('x');e.stopPropagation();d.dispatchEvent(e);const first=log.join();\
        d.dispatchEvent(e);let again='';\
        d.addEventListener('y',ev=>{try{d.dispatchEvent(ev)}catch(x){again=x.name}});d.dispatchEvent(new Event('y'));\
        const r=[first,log.join(),e.cancelBubble,again].join('|');";
    assert_eq!(eval(source, "r"), "|capture|false|InvalidStateError");
}

/** isEqualNode compares type, name, attributes, data and children; isSameNode identity. */
#[test]
fn is_equal_node_compares_structure() {
    let source = "const m=h=>{const d=document.createElement('div');d.innerHTML=h;return d};\
        const a=m('<p class=x id=y>t</p>'),b=m('<p id=y class=x>t</p>'),c=m('<p id=y>t</p>'),d=m('<p class=x id=y>u</p>');\
        const r=[a.isEqualNode(b),a.isEqualNode(c),a.isEqualNode(d),a.isSameNode(b),a.isEqualNode(null)].join();";
    assert_eq!(eval(source, "r"), "true,false,false,false,false");
}

/** normalize removes empty text nodes and merges adjacent ones, in the whole subtree. */
#[test]
fn normalize_merges_text_runs() {
    let source = "const d=document.createElement('div');const t=s=>document.createTextNode(s);\
        d.append(t('a'),t(''),t('b'),document.createElement('i'),t(''));d.lastChild.previousSibling.append(t('x'),t('y'));\
        const first=d.firstChild;d.normalize();\
        const r=[d.childNodes.length,d.firstChild===first,first.data,d.lastChild.childNodes.length,d.lastChild.textContent].join();";
    assert_eq!(eval(source, "r"), "2,true,ab,1,xy");
}
