/** Markup admission is host policy on each request, never an eval permission or saved-state grant. */
mod common;
use mars_v9::{HandlerHash, HandlerPolicy, HandlerRestriction, PageEnvironment};

/** A current request with both compilation surfaces independently constrained. */
fn environment(allow_inline: bool) -> PageEnvironment {
    PageEnvironment {
        dom_level: mars_v9::DomLevel::Extended,
        disallow_code_generation_from_strings: true,
        body_attributes: [
            ("onclick".into(), "r+='attribute'".into()),
            ("onload".into(), "r+='load'".into()),
        ]
        .into(),
        handler_policy: HandlerPolicy {
            restrictions: vec![HandlerRestriction {
                allow_inline,
                hashes: vec![],
            }],
        },
        ..common::environment()
    }
}

/** Click attributes and body-forwarded onload use the same markup-only check. */
#[test]
fn attribute_policy_covers_click_and_forwarded_body_load() {
    let source = "let r='';document.body.dispatchEvent(new Event('click'));window.dispatchEvent(new Event('load'));";
    for (allowed, expected) in [(false, ""), (true, "attributeload")] {
        let result = common::load_with(source, &environment(allowed));
        assert!(result.failed.is_empty(), "{:?}", result.failed);
        assert_eq!(common::global(&result, "r"), expected);
    }
}

/** Policy hashes cover raw source and all effective restrictions must match it. */
#[test]
fn hashes_match_raw_source_and_intersect_policies() {
    let source = "r+='hash'";
    let hash = ring::digest::digest(&ring::digest::SHA256, source.as_bytes());
    let mut environment = environment(false);
    environment.handler_policy.restrictions[0]
        .hashes
        .push(HandlerHash::Sha256(hash.as_ref().to_vec()));
    environment
        .body_attributes
        .insert("onclick".into(), source.into());
    let script = "let r='';document.body.dispatchEvent(new Event('click'))";
    let result = common::load_with(script, &environment);
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    assert_eq!(common::global(&result, "r"), "hash");
    environment
        .handler_policy
        .restrictions
        .push(HandlerRestriction::default());
    assert_eq!(
        common::global(&common::load_with(script, &environment), "r"),
        ""
    );
}

/** A restored callback uses the new request's policy, not an earlier permissive environment. */
#[test]
fn restored_tasks_recheck_markup_policy_without_blocking_listeners() {
    let sources = vec!["let r='';const b=document.body;b.addEventListener('click',()=>r+='listener');setTimeout(()=>b.dispatchEvent(new Event('click')),0)".into()];
    let host = common::host();
    let initial = environment(true);
    let loaded = mars_v9::load_page(common::input(&sources, &host, &initial)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    let denied = environment(false);
    let result = mars_v9::run_timers(
        common::input(&sources, &host, &denied),
        common::round_trip(loaded.state),
        0.0,
        1,
    )
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    assert_eq!(common::global(&result, "r"), "listener");
}

/** The existing DOM mutation refusal for handler attributes remains stricter than CSP. */
#[test]
fn dynamic_handler_attribute_mutation_remains_explicitly_unsupported() {
    let result = common::load_with(
        "document.getElementById('go').setAttribute('onclick','throw 1')",
        &environment(true),
    );
    assert!(result.failed.iter().any(|failure| {
        failure
            .error
            .message
            .contains("Scripts may not write event handler")
    }));
}

/** A permitted handler cannot make string timers bypass the dynamic-code gate. */
#[test]
fn permitted_handler_keeps_string_timer_codegen_blocked() {
    let mut environment = environment(true);
    environment.body_attributes.insert(
        "onclick".into(),
        "setTimeout(\"r+='timer'\",0);r+='attribute'".into(),
    );
    let sources = vec!["let r='';document.body.dispatchEvent(new Event('click'))".into()];
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    let result = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        loaded.state,
        0.0,
        1,
    )
    .unwrap();
    assert_eq!(common::global(&result, "r"), "attribute");
    assert!(result.effects.console.iter().any(|entry| {
        entry
            .text
            .contains("Code generation from strings disallowed")
    }));
}

/** Invalid policy records fail the public page entry point before any script runs. */
#[test]
fn malformed_and_oversized_host_policies_are_rejected() {
    let host = common::host();
    let sources = vec!["throw 'must not execute'".into()];
    let malformed = HandlerRestriction {
        allow_inline: false,
        hashes: vec![HandlerHash::Sha256(vec![])],
    };
    for restrictions in [
        vec![malformed],
        vec![HandlerRestriction::default(); mars_v9::HANDLER_POLICY_LIMIT + 1],
        vec![HandlerRestriction {
            allow_inline: true,
            hashes: vec![HandlerHash::Sha256(vec![]); mars_v9::HANDLER_POLICY_LIMIT + 1],
        }],
    ] {
        let mut environment = environment(false);
        environment.handler_policy.restrictions = restrictions;
        let wire = serde_json::to_value(&environment).unwrap();
        let restored = serde_json::from_value(wire).unwrap();
        assert!(mars_v9::load_page(common::input(&sources, &host, &restored)).is_err());
    }
    assert!(serde_json::from_value::<HandlerHash>(serde_json::json!({"Unknown":[]})).is_err());
}
