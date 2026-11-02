/** Frame lookups exercise ordinary JavaScript access and immutable context incarnations. */
use crate::common;
use mars_v9::{FrameRoute, MAX_FRAME_WINDOW_ID, PageEnvironment, sibling_target, sibling_window};

/** Three immediate siblings, including the caller, reproduce a parent.frames handshake. */
fn child_environment() -> PageEnvironment {
    PageEnvironment {
        embedder_origin: Some("https://parent.test".into()),
        own_window: Some(12),
        parent_frames: vec![
            route(1, 11, "a-recaptcha"),
            route(2, 12, "caller"),
            route(3, 13, "other"),
        ],
        ..common::environment()
    }
}

/** Names belong to embedding attributes, while window identities never depend on order. */
fn route(owner: usize, window: i64, name: &str) -> FrameRoute {
    FrameRoute {
        owner,
        window,
        name: name.into(),
    }
}

/** Owner attribute mutations use the native browser's extended projection contract. */
fn local_environment() -> PageEnvironment {
    PageEnvironment {
        dom_level: mars_v9::DomLevel::Extended,
        ..common::environment()
    }
}

/** A script assertion result is read through the public VM execution result. */
fn result(source: &str, environment: &PageEnvironment) -> String {
    let execution = common::load_with(source, environment);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    common::global(&execution, "result")
}

#[test]
fn named_and_indexed_siblings_share_identity_and_self_is_the_real_window() {
    assert_eq!(
        result(
            "const result=[parent.frames===parent,parent.length,parent.frames['a-recaptcha']===parent.frames[0],parent.frames[1]===window,parent.frames[2]!==parent.frames[0],parent.frames[3]===undefined,parent.frames['00']===undefined].join(',');",
            &child_environment(),
        ),
        "true,3,true,true,true,true,true"
    );
}

#[test]
fn sibling_messages_use_context_identities_without_exposing_remote_documents() {
    let execution = common::load_with(
        "parent.frames['a-recaptcha'].postMessage('hello','https://sibling.test');const result=typeof parent.frames[0].document;",
        &child_environment(),
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(common::global(&execution, "result"), "undefined");
    assert_eq!(execution.effects.messages.len(), 1);
    assert_eq!(
        execution.effects.messages[0].target,
        sibling_target(11).unwrap()
    );
    assert_eq!(
        execution.effects.messages[0].target_origin,
        "https://sibling.test"
    );
}

#[test]
fn duplicate_names_choose_the_first_context_and_builtins_keep_precedence() {
    let mut environment = child_environment();
    environment.parent_frames[1].name = "a-recaptcha".into();
    environment.parent_frames[2].name = "postMessage".into();
    assert_eq!(
        result(
            "const result=[parent['a-recaptcha']===parent[0],typeof parent.postMessage,parent.frames===parent].join(',');",
            &environment,
        ),
        "true,function,true"
    );
}

#[test]
fn changing_child_window_name_does_not_introduce_a_parent_property() {
    assert_eq!(
        result(
            "window.name='invented';const result=[parent.invented===undefined,parent.caller===window].join(',');",
            &child_environment(),
        ),
        "true,true"
    );
}

#[test]
fn local_indexed_access_includes_blank_frames_and_same_turn_insertions() {
    assert_eq!(
        result(
            "const first=document.createElement('iframe');first.name='first';document.body.append(first);const second=document.createElement('iframe');second.src='https://other.test/';document.body.append(second);const result=[frames.length,frames[0]===first.contentWindow,frames.first===first.contentWindow,frames[1]===second.contentWindow].join(',');",
            &local_environment(),
        ),
        "2,true,true,true"
    );
}

#[test]
fn owner_rename_and_removal_update_lookup_immediately() {
    assert_eq!(
        result(
            "const f=document.createElement('iframe');f.name='before';f.src='https://other.test/';document.body.append(f);f.name='after';const named=frames.before===undefined&&frames.after===f.contentWindow;f.remove();const result=[named,frames.length,frames.after===undefined,frames[0]===undefined].join(',');",
            &local_environment(),
        ),
        "true,0,true,true"
    );
}

#[test]
fn retained_proxy_cannot_send_to_a_same_element_reattachment() {
    let execution = common::load_with(
        "const f=document.createElement('iframe');f.src='https://other.test/';document.body.append(f);const old=f.contentWindow;f.remove();document.body.append(f);const fresh=f.contentWindow;old.postMessage('stale','*');fresh.postMessage('current','*');const result=old!==fresh;",
        &local_environment(),
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(common::global(&execution, "result"), "true");
    assert_eq!(execution.effects.messages.len(), 1);
    assert_eq!(execution.effects.messages[0].target_incarnation, 1);
    assert!(!execution.effects.frame_detaches.is_empty());
}

#[test]
fn removing_a_container_retires_each_nested_iframe_context() {
    let execution = common::load_with(
        "const host=document.createElement('div');const f=document.createElement('iframe');f.src='https://other.test/';host.append(f);document.body.append(host);const old=f.contentWindow;host.remove();old.postMessage('stale','*');const result=frames.length;",
        &local_environment(),
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(common::global(&execution, "result"), "0");
    assert!(execution.effects.messages.is_empty());
    assert_eq!(execution.effects.frame_detaches.len(), 1);
}

#[test]
fn replacing_projected_container_text_retires_frame_proxies() {
    let execution = common::load_with(
        "const host=document.getElementById('main');const f=document.createElement('iframe');f.src='https://other.test/';host.append(f);const old=f.contentWindow;host.textContent='removed';document.body.append(f);old.postMessage('stale','*');f.contentWindow.postMessage('fresh','*');const result=old!==f.contentWindow;",
        &local_environment(),
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(common::global(&execution, "result"), "true");
    assert_eq!(execution.effects.messages.len(), 1);
    assert!(!execution.effects.frame_detaches.is_empty());
}

#[test]
fn sibling_encoding_excludes_owner_and_parent_domains_and_round_trips_exactly() {
    for window in [1, 12, MAX_FRAME_WINDOW_ID] {
        let target = sibling_target(window).unwrap();
        assert!(target < -1);
        assert_eq!(sibling_window(target), Some(window));
        assert_eq!((target as f64) as i64, target);
    }
    for window in [-1, 0, MAX_FRAME_WINDOW_ID + 1] {
        assert_eq!(sibling_target(window), None);
    }
    for target in [i64::MIN, -1, 0, 1] {
        assert_eq!(sibling_window(target), None);
    }
}

#[test]
fn forged_parent_directories_are_rejected_before_script_execution() {
    let base = child_environment();
    for case in 0..7 {
        let mut environment = base.clone();
        match case {
            0 => environment.parent_frames[0].window = 0,
            1 => environment.parent_frames[0].window = 12,
            2 => environment.parent_frames[0].owner = 2,
            3 => environment.own_window = Some(44),
            4 => environment.own_window = None,
            5 => environment.embedder_origin = None,
            _ => environment.parent_frames[0].name = "a".repeat(mars_v9::constants::TEXT_BYTES + 1),
        }
        let host = common::host();
        let error = mars_v9::load_page(common::input(&[], &host, &environment))
            .err()
            .unwrap();
        assert_eq!(error.code, "invalid_request", "case {case}");
    }
}

#[test]
fn old_environments_default_to_an_empty_directory() {
    let mut wire = serde_json::to_value(common::environment()).unwrap();
    wire.as_object_mut().unwrap().remove("parent_frames");
    wire.as_object_mut().unwrap().remove("own_window");
    let environment: PageEnvironment = serde_json::from_value(wire).unwrap();
    assert!(environment.parent_frames.is_empty());
    assert!(environment.own_window.is_none());
}

#[test]
fn duplicate_and_unsupported_transfer_entries_leave_the_valid_port_attached() {
    for list in ["[c.port1,c.port1]", "[c.port1,{}]", "[c.port1,c.port2]"] {
        let source = format!(
            "const c=new MessageChannel();let rejected=false;try{{parent[0].postMessage('bad','*',{list})}}catch(e){{rejected=true}}parent[0].postMessage('good','*',[c.port1]);const result=rejected;"
        );
        let execution = common::load_with(&source, &child_environment());
        assert!(execution.failed.is_empty(), "{:?}", execution.failed);
        assert_eq!(common::global(&execution, "result"), "true");
        assert_eq!(execution.effects.messages.len(), 1);
        assert_eq!(execution.effects.messages[0].ports.len(), 1);
    }
}

#[test]
fn an_already_transferred_entry_does_not_detach_an_earlier_valid_entry() {
    let execution = common::load_with(
        "const a=new MessageChannel();const b=new MessageChannel();parent[0].postMessage('first','*',[b.port1]);let rejected=false;try{parent[0].postMessage('bad','*',[a.port1,b.port1])}catch(e){rejected=true}parent[0].postMessage('last','*',[a.port1]);const result=rejected;",
        &child_environment(),
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(common::global(&execution, "result"), "true");
    assert_eq!(execution.effects.messages.len(), 2);
}

/** Three native contexts describe a grandchild without granting access to ancestor documents. */
fn nested_environment() -> PageEnvironment {
    use mars_v9::FrameContext;
    PageEnvironment {
        context_window: Some(13),
        own_window: Some(13),
        embedder_origin: Some("https://parent.test".into()),
        parent_frames: vec![route(1, 13, "grand")],
        frame_contexts: vec![
            FrameContext {
                window: 11,
                parent: None,
                children: vec![route(1, 12, "child")],
            },
            FrameContext {
                window: 12,
                parent: Some(11),
                children: vec![route(1, 13, "grand")],
            },
            FrameContext {
                window: 13,
                parent: Some(12),
                children: vec![],
            },
        ],
        ..common::environment()
    }
}

/** Native context relationships distinguish parent from top while all aliases share proxies. */
#[test]
fn nested_relationships_preserve_global_identity_and_local_owner_namespaces() {
    assert_eq!(
        result(
            "const result=[parent!==top,parent.parent===top,top.parent===top,top.frames[0]===parent,parent.frames[0]===window,top.frames.child===parent,parent.document===undefined,top.document===undefined].join(',');",
            &nested_environment()
        ),
        "true,true,true,true,true,true,true,true"
    );
}

/** The two ancestor targets are separate native windows, never the same parent sentinel. */
#[test]
fn nested_messages_address_distinct_immediate_and_root_ancestors() {
    let execution = common::load_with(
        "parent.postMessage('parent','*');top.postMessage('top','*');",
        &nested_environment(),
    );
    assert!(execution.failed.is_empty());
    assert_eq!(execution.effects.messages[0].target, -1);
    assert_eq!(
        execution.effects.messages[1].target,
        sibling_target(11).unwrap()
    );
}

/** Forged cycles, missing parents and reused windows fail before page code can run. */
#[test]
fn malformed_nested_context_trees_are_rejected() {
    for case in 0..6 {
        let mut environment = nested_environment();
        match case {
            0 => environment.frame_contexts[0].parent = Some(13),
            1 => environment.frame_contexts[2].parent = Some(99),
            2 => environment.frame_contexts[1].window = 11,
            3 => environment.context_window = Some(99),
            4 => environment.frame_contexts[1].children[0].window = 11,
            _ => environment.frame_contexts[0].children[0].window = 13,
        }
        let host = common::host();
        let error = mars_v9::load_page(common::input(&[], &host, &environment))
            .err()
            .unwrap();
        assert_eq!(error.code, "invalid_request", "case {case}");
    }
}
