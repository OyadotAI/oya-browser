/** The indexed live-range and listener registries behave as the DOM standard says through script:
range points follow subtree removals and insertions, and listeners match, fire and unregister by key. */
use crate::common;
use common::eval;

/** A removed subtree's points are found by walking it: a point two levels down collapses to the gap. */
#[test]
fn removing_a_deep_subtree_moves_points_found_by_the_walk() {
    let source = "const d=document.createElement('div');const a=document.createElement('a');const b=document.createElement('b');\
        d.append(a);a.append(b);const t=document.createTextNode('xyz');b.append(t);\
        const r=document.createRange();r.setStart(t,2);r.setEnd(t,3);\
        d.removeChild(a);const out=[r.startContainer===d,r.startOffset,r.endContainer===d,r.endOffset].join();";
    assert_eq!(eval(source, "out"), "true,0,true,0");
}

/** A point in an untouched sibling range is not moved by a removal elsewhere in the tree. */
#[test]
fn removal_leaves_ranges_outside_the_removed_subtree_alone() {
    let source = "const d=document.createElement('div');const a=document.createElement('a');const c=document.createElement('c');\
        d.append(a,c);const r=document.createRange();r.setStart(c,0);r.setEnd(c,0);\
        d.removeChild(a);const out=[r.startContainer===c,r.startOffset].join();";
    assert_eq!(eval(source, "out"), "true,0");
}

/** A range moved to a new container is found by the next mutation at that container. */
#[test]
fn range_moved_to_a_new_container_follows_later_insertions() {
    let source = "const d=document.createElement('div');const a=document.createElement('a');const c=document.createElement('c');\
        d.append(a,c);const r=document.createRange();r.setStart(a,0);r.setEnd(a,0);r.setStart(c,0);\
        d.insertBefore(document.createElement('i'),c);const out=[r.startContainer===c,r.startOffset].join();";
    assert_eq!(eval(source, "out"), "true,0");
}

/** Insertions before a parent point move it past each new node. */
#[test]
fn insertion_moves_parent_points_after_the_insertion_point() {
    let source = "const d=document.createElement('div');const a=document.createElement('a');d.append(a);\
        const r=document.createRange();r.setStart(d,1);r.setEnd(d,1);\
        d.insertBefore(document.createElement('i'),a);d.insertBefore(document.createElement('j'),a);\
        const out=[r.startOffset,r.endOffset].join();";
    assert_eq!(eval(source, "out"), "3,3");
}

/** Adding the same callback twice for one type, target and capture registers it once. */
#[test]
fn duplicate_add_listener_registers_once() {
    let source = "let n=0;const f=()=>{n++};document.body.addEventListener('x',f);document.body.addEventListener('x',f);\
        document.body.dispatchEvent(new Event('x'));const out=n;";
    assert_eq!(eval(source, "out"), "1");
}

/** Removing with the same capture flag drops the registration; a different capture flag leaves it. */
#[test]
fn remove_listener_matches_on_capture_flag() {
    let source = "let n=0;const f=()=>{n++};document.body.addEventListener('x',f,true);\
        document.body.removeEventListener('x',f,false);document.body.dispatchEvent(new Event('x'));\
        const before=n;document.body.removeEventListener('x',f,true);document.body.dispatchEvent(new Event('x'));const out=before+','+n;";
    assert_eq!(eval(source, "out"), "1,1");
}

/** A once listener runs on the first dispatch only. */
#[test]
fn once_listener_runs_only_on_the_first_dispatch() {
    let source = "let n=0;document.body.addEventListener('x',()=>{n++},{once:true});\
        document.body.dispatchEvent(new Event('x'));document.body.dispatchEvent(new Event('x'));const out=n;";
    assert_eq!(eval(source, "out"), "1");
}

/** A listener removed by an earlier listener during the same dispatch does not run. */
#[test]
fn listener_removed_mid_dispatch_does_not_run() {
    let source = "let n=0;const second=()=>{n++};document.body.addEventListener('x',()=>{document.body.removeEventListener('x',second)});\
        document.body.addEventListener('x',second);document.body.dispatchEvent(new Event('x'));const out=n;";
    assert_eq!(eval(source, "out"), "0");
}

/** A callback that is not an object is a TypeError, as WebIDL's EventListener conversion requires. */
#[test]
fn non_object_listener_callback_throws_type_error() {
    let source = "let name='none';try{document.body.addEventListener('x',5)}catch(e){name=e.name}const out=name;";
    assert_eq!(eval(source, "out"), "TypeError");
}

/** Listeners on one target and type fire in the order they were added. */
#[test]
fn listeners_fire_in_registration_order() {
    let source = "let s='';document.body.addEventListener('x',()=>{s+='a'});document.body.addEventListener('x',()=>{s+='b'});\
        document.body.addEventListener('x',()=>{s+='c'});document.body.dispatchEvent(new Event('x'));const out=s;";
    assert_eq!(eval(source, "out"), "abc");
}
