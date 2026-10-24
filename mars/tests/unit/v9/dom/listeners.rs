/** Public projected-DOM listeners rules use the shared real-document fixture. */
use super::*;

/** Removing a pending registration skips it; removing and readding creates an ID outside this snapshot. */
#[test]
fn removal_during_dispatch_skips_pending_callbacks_even_when_the_same_function_is_readded() {
    let source = "const b=document.getElementById('button');let log='';function pending(){log=log+'p';}b.addEventListener('click',()=>{log=log+'a';b.removeEventListener('click',pending);b.addEventListener('click',pending);});b.addEventListener('click',pending);b.addEventListener('click',()=>{document.getElementById('out').textContent=log;});";
    let document = page();
    let first = click(source, load(source, &document).state, &document);
    assert_eq!(first.changes[0].text().unwrap(), "a");
    let second = click(source, first.state, &document);
    assert_eq!(second.changes[0].text().unwrap(), "aa");
}

/** Inline removal affects an already-taken listener snapshot without executing the removed callback. */
#[test]
fn inline_handlers_can_remove_callbacks_from_the_pending_snapshot() {
    let document =
        document("<p id=out></p><button id=button type=button onclick='inline()'>Click</button>");
    let source = "const b=document.getElementById('button');let log='';function pending(){log=log+'p';}function inline(){log=log+'i';b.removeEventListener('click',pending);}b.addEventListener('click',pending);b.addEventListener('click',()=>{document.getElementById('out').textContent=log;});";
    assert_eq!(
        click(source, load(source, &document).state, &document).changes[0]
            .text()
            .unwrap(),
        "i"
    );
}

/** Ordinary callbacks receive the button; arrows retain lexical receivers and bound callbacks retain their own. */
#[test]
fn ordinary_arrow_and_bound_callbacks_use_their_supported_receiver_rules() {
    let source = "const button=document.getElementById('button');const out=document.getElementById('out');let value='';button.addEventListener('click',function(e){value=value+(this===button)+' '+(e.target===button)+' ';});const maker={make:function(){button.addEventListener('click',()=>{value=value+(this===maker)+' ';});}};maker.make();button.addEventListener('click',function(e){out.textContent=value+this.name+' '+e.type;}.bind({name:'bound'}));";
    let executed = click(source, load(source, &page()).state, &page());
    assert_eq!(
        executed.changes[0].text().unwrap(),
        "true true true bound click"
    );
}

/** Registration itself roots anonymous callbacks and their captured state through request serialization and collection. */
#[test]
fn callbacks_reachable_only_from_registration_survive_collection_and_repeat_clicks() {
    let source = "function install(){let count=0;document.getElementById('button').addEventListener('click',function(){count++;document.getElementById('out').textContent=count;});}install();";
    let document = page();
    let mut state = load(source, &document).state;
    assert!(state.has_click_listener("page-a", document.get_by_id("button").unwrap()));
    assert!(!state.has_click_listener("another-page", document.get_by_id("button").unwrap()));
    for expected in 1..=3 {
        let executed = click(source, state, &document);
        assert_eq!(executed.changes[0].text().unwrap(), expected.to_string());
        state = executed.state;
    }
}

/** Duplicate registrations compare original identity; removal and readdition move a listener to the tail. */
#[test]
fn duplicate_removal_readdition_and_dispatch_additions_follow_ordered_snapshots() {
    let source = "const b=document.getElementById('button');let log='';function first(){log=log+'a';b.removeEventListener('click',last);b.addEventListener('click',added);}function last(){log=log+'z';}function added(){log=log+'n';}b.addEventListener('click',first);b.addEventListener('click',first);b.addEventListener('click',last);b.removeEventListener('click',first);b.addEventListener('click',first);b.addEventListener('click',()=>{document.getElementById('out').textContent=log;});";
    let document = page();
    let first = click(source, load(source, &document).state, &document);
    assert_eq!(first.changes[0].text().unwrap(), "za");
    let second = click(source, first.state, &document);
    assert_eq!(second.changes[0].text().unwrap(), "zaa");
    let third = click(source, second.state, &document);
    assert_eq!(third.changes[0].text().unwrap(), "zaana");
}

/** Inline code runs first but cannot insert newly registered listeners into the active dispatch snapshot. */
#[test]
fn inline_handlers_run_before_the_preexisting_listener_snapshot() {
    let document =
        document("<p id=out></p><button id=button type=button onclick='inline()'>Click</button>");
    let source = "const b=document.getElementById('button');let log='';function late(){log=log+'n';}function inline(){log=log+'i';b.addEventListener('click',late);}b.addEventListener('click',()=>{log=log+'a';document.getElementById('out').textContent=log;});";
    let first = click(source, load(source, &document).state, &document);
    assert_eq!(first.changes[0].text().unwrap(), "ia");
    let second = click(source, first.state, &document);
    assert_eq!(second.changes[0].text().unwrap(), "iaia");
}

/** An event is the sole argument, retains identity and target, and loses currentTarget after dispatch. */
#[test]
fn retained_event_properties_and_cancellation_follow_dispatch_lifetime() {
    let source = "const b=document.getElementById('button');let saved=null;let previous='';b.addEventListener('click',function(e,extra){if(saved!==null){previous=(saved.currentTarget===null)+' '+saved.defaultPrevented+' ';}saved=e;const initial=e.defaultPrevented;e.preventDefault();document.getElementById('out').textContent=previous+e.type+' '+(e.target===b)+' '+(e.currentTarget===b)+' '+initial+' '+e.defaultPrevented+' '+(extra===undefined);});";
    let document = page();
    let first = click(source, load(source, &document).state, &document);
    assert_eq!(
        first.changes[0].text().unwrap(),
        "click true true false true true"
    );
    let second = click(source, first.state, &document);
    assert_eq!(
        second.changes[0].text().unwrap(),
        "true true click true true false true true"
    );
}

/** Event metadata is readonly (sloppy writes are ignored) and stopPropagation keeps the click from bubbling. */
#[test]
fn event_properties_are_readonly_and_propagation_can_be_stopped() {
    let source = "let log='';document.addEventListener('click',()=>{log+='d';document.getElementById('out').textContent=log;});document.getElementById('button').addEventListener('click',function(e){e.type='x';e.target=null;e.currentTarget=null;e.defaultPrevented=true;log+=e.type+' '+(e.target===this)+' '+e.defaultPrevented;e.stopPropagation();document.getElementById('out').textContent=log;});";
    let document = page();
    let clicked = click(source, load(source, &document).state, &document);
    assert_eq!(
        clicked.changes.last().unwrap().text().unwrap(),
        "click true false"
    );
}

/** Listener options, other event types and handleEvent objects register; nullish callbacks are no-ops. */
#[test]
fn listener_registration_accepts_options_types_and_listener_objects() {
    assert_eq!(
        result(
            "const b=document.getElementById('button');b.addEventListener('click',null);b.addEventListener('click',undefined);",
            "1"
        ),
        "1"
    );
    let source = "let n=0;const b=document.getElementById('button');b.addEventListener('click',()=>n++,false);b.addEventListener('click',()=>n++,{});b.addEventListener('change',()=>{n+=100;});b.addEventListener('click',{handleEvent(){n++;document.getElementById('out').textContent=n;}});";
    let document = page();
    let clicked = click(source, load(source, &document).state, &document);
    assert_eq!(clicked.changes[0].text().unwrap(), "3");
}
