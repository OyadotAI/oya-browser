/** Popover mutations preserve effective types, reentrant hiding, private visibility and selector semantics. */
use crate::common;
use mars_v9::DomLevel;

/** Evaluate DOM behavior through the public page entry point with extended element bindings enabled. */
fn eval(source: &str) -> String {
    let mut environment = common::environment();
    environment.dom_level = DomLevel::Extended;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    common::global(&execution, "r")
}

/** The content value changes first, while beforetoggle observes the still-open previous internal type. */
#[test]
fn idl_type_changes_hide_synchronously_with_beforetoggle() {
    assert_eq!(
        eval(
            "const p=document.createElement('div');p.popover='manual';document.body.append(p);p.showPopover();let log='';p.addEventListener('beforetoggle',e=>{log+=e.oldState+'>'+e.newState+','+e.cancelable+','+p.popover+','+p.matches(':popover-open')});p.popover='auto';let r=log+','+p.matches(':popover-open');"
        ),
        "open>closed,false,auto,true,false"
    );
}

/** Equivalent normalized values leave an open popover alone, even if the content text differs. */
#[test]
fn unchanged_effective_types_do_not_hide() {
    assert_eq!(
        eval(
            "const p=document.createElement('div');p.popover='manual';document.body.append(p);p.showPopover();let calls=0;p.addEventListener('beforetoggle',()=>calls++);p.popover='bogus';const a=p.matches(':popover-open');p.hidePopover();p.popover='auto';p.showPopover();calls=0;p.setAttribute('PoPoVeR','AuTo');let r=a+','+p.matches(':popover-open')+','+calls+','+p.getAttribute('popover');"
        ),
        "true,true,0,AuTo"
    );
}

/** All public content-attribute write paths use the same hide algorithm. */
#[test]
fn set_remove_and_toggle_attribute_hide_open_popovers() {
    assert_eq!(
        eval(
            "const p=document.createElement('div');document.body.append(p);let calls=0;p.addEventListener('beforetoggle',e=>{if(e.newState==='closed')calls++});const log=[];for(const change of [()=>p.setAttribute('popover','manual'),()=>p.removeAttribute('popover'),()=>p.toggleAttribute('popover',false)]){p.popover='auto';p.showPopover();change();log.push(p.matches(':popover-open'))}let r=log.join()+','+calls;"
        ),
        "false,false,false,3"
    );
}

/** A nullable reflected DOMString treats both nullish values as removal. */
#[test]
fn nullish_idl_values_remove_the_attribute_and_hide() {
    assert_eq!(
        eval(
            "const p=document.createElement('div');document.body.append(p);const log=[];for(const value of [null,undefined]){p.popover='auto';p.showPopover();p.popover=value;log.push(p.popover===null,!p.hasAttribute('popover'),!p.matches(':popover-open'))}let r=log.join();"
        ),
        "true,true,true,true,true,true"
    );
}

/** A type change from a closing callback suppresses another event dispatch rather than recursing. */
#[test]
fn reentrant_type_mutation_hides_without_recursive_events() {
    assert_eq!(
        eval(
            "const p=document.createElement('div');p.popover='auto';document.body.append(p);p.showPopover();let calls=0;p.addEventListener('beforetoggle',e=>{if(e.newState==='closed'){calls++;p.popover='manual'}});p.popover='hint';let r=calls+','+p.popover+','+p.matches(':popover-open');"
        ),
        "1,manual,false"
    );
}

/** A hide already in progress cannot close the newer opening created by its callback. */
#[test]
fn reentrant_hide_and_reopen_preserves_the_new_opening() {
    assert_eq!(
        eval(
            "const p=document.createElement('div');p.popover='manual';document.body.append(p);p.showPopover();let calls=0;p.addEventListener('beforetoggle',e=>{if(e.newState==='closed'){calls++;p.hidePopover();p.showPopover()}},{once:true});p.hidePopover();let r=calls+','+p.matches(':popover-open');"
        ),
        "1,true"
    );
}

/** Removing an ancestor closes showing descendants without dispatching hiding events. */
#[test]
fn disconnected_popovers_close_without_beforetoggle() {
    assert_eq!(
        eval(
            "const parent=document.createElement('section'),p=document.createElement('div'),child=document.createElement('div');p.popover='auto';child.popover='auto';p.append(child);parent.append(p);document.body.append(parent);p.showPopover();child.showPopover();let calls=0;p.addEventListener('beforetoggle',()=>calls++);child.addEventListener('beforetoggle',()=>calls++);parent.remove();const a=!p.matches(':popover-open')&&!child.matches(':popover-open');document.body.append(parent);let r=a+','+!p.matches(':popover-open')+','+!child.matches(':popover-open')+','+calls;"
        ),
        "true,true,true,0"
    );
}

/** The selector reads private state through compound, nested and relational selectors without an attribute shim. */
#[test]
fn popover_open_selectors_follow_real_state_through_composition() {
    assert_eq!(
        eval(
            "const p=document.createElement('div'),child=document.createElement('span');p.id='pop';p.popover='manual';p.append(child);document.body.append(p);const log=[p.matches(':not(:popover-open)')];p.showPopover();log.push(document.querySelector('div:popover-open')===p,document.querySelectorAll(':is(:popover-open)').length===1,document.body.matches(':has(:popover-open)'),child.closest(':popover-open')===p,!p.hasAttribute('popoverShowing'));p.hidePopover();log.push(document.querySelector(':popover-open')===null);let r=log.join();"
        ),
        "true,true,true,true,true,true,true"
    );
}

/** A later animation frame may reopen the element after a type-change callback removes it. */
#[test]
fn type_change_removal_then_animation_frame_reopen_uses_the_new_type() {
    let sources=vec!["const p=document.createElement('div');p.popover='manual';document.body.append(p);p.showPopover();p.addEventListener('beforetoggle',e=>{if(e.newState==='closed'){p.remove();requestAnimationFrame(()=>{document.body.append(p);p.showPopover();console.log(p.popover,p.matches(':popover-open'))})}},{once:true});p.popover='auto';console.log('sync',p.matches(':popover-open'));".to_owned()];
    let host = common::host();
    let mut environment = common::environment();
    environment.dom_level = DomLevel::Extended;
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::console(&loaded), ["sync false"]);
    let resumed = mars_v9::run_timers(input(), loaded.state, 50.0, 10).unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::console(&resumed), ["auto true"]);
}

/** The original show operation detects a type changed by another popover's closing callback. */
#[test]
fn type_change_while_hiding_another_popover_rejects_the_show() {
    assert_eq!(
        eval(
            "const a=document.createElement('div'),b=document.createElement('div');a.popover='auto';b.popover='auto';document.body.append(a,b);a.showPopover();a.addEventListener('beforetoggle',e=>{if(e.newState==='closed')b.popover='manual'},{once:true});let error='none';try{b.showPopover()}catch(e){error=e.name}let r=error+','+a.matches(':popover-open')+','+b.matches(':popover-open')+','+b.popover;"
        ),
        "InvalidStateError,false,false,manual"
    );
}

/** Saved wrappers keep their open type and selector visibility until a later request changes the attribute. */
#[test]
fn open_type_and_visibility_survive_state_transport() {
    let sources=vec!["const p=document.getElementById('first');p.showPopover();document.getElementById('go').addEventListener('click',()=>{console.log(p.matches(':popover-open'));p.popover='auto';console.log(p.matches(':popover-open'))});".to_owned()];
    let host = common::host();
    let mut environment = common::environment();
    environment.dom_level = DomLevel::Extended;
    let mut document = common::page();
    document.nodes[1]
        .attributes
        .insert("popover".to_owned(), "manual".to_owned());
    let input = || mars_v9::PageInput {
        document: document.clone(),
        ..common::input(&sources, &host, &environment)
    };
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(input(), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true", "false"]);
}
