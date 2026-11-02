/** Parser-created scripts report resource completion independently of JavaScript success. */
use mars::{document, v9};

/** Run three ordered sources through the native fragment host and apply their mutations. */
fn run(payload: &str, refused: bool) -> v9::Execution {
    let document = document::parse("<div id=out></div>").unwrap();
    let host = v9::DocumentHost {
        imports: None,
        canvas: &mars_core::canvas::Unavailable,
        languages: &[],
        fragments: &document::DomFragments,
    };
    let attributes = [("id", "payload"), ("onload", "window.loads++;document.getElementById('out').textContent=String(window.payload);if(document.currentScript!==null)throw Error('currentScript');"), ("onerror", "window.errors++")]
        .into_iter().map(|(name, value)| (name.into(), value.into())).collect();
    let environment = v9::PageEnvironment {
        dom_level: v9::DomLevel::Extended,
        scripts: vec![
            String::new(),
            "https://example.test/payload.js".into(),
            String::new(),
        ],
        script_attributes: vec![Default::default(), attributes, Default::default()],
        refused_scripts: if refused { vec![1] } else { vec![] },
        ..Default::default()
    };
    let sources = [
        "window.loads=0;window.errors=0;window.payload=0".into(),
        payload.into(),
        "console.log(window.loads+':'+window.errors+':'+window.payload)".into(),
    ];
    let execution = v9::load_page(v9::PageInput {
        sources: &sources,
        source_keys: &[],
        module_keys: &[],
        document: document::script_document(&document, "parsed-scripts").unwrap(),
        storage: Default::default(),
        host: &host,
        environment: &environment,
        modules: v9::ModuleInput::NONE,
    })
    .unwrap();
    document::apply_script_changes(&document, &execution.script_changes).unwrap();
    execution
}

/** The next script observes completion, including the payload's mutations. */
#[test]
fn fetched_script_fires_load_before_the_next_source() {
    let execution = run("window.payload=42", false);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(execution.effects.console.last().unwrap().text, "1:0:42");
}

/** Empty downloaded source is successful and is distinct from failed acquisition. */
#[test]
fn empty_script_fires_load() {
    assert_eq!(run("", false).effects.console.last().unwrap().text, "1:0:0");
}

/** JavaScript exceptions report failure while successful resource acquisition still fires load. */
#[test]
fn throwing_script_still_fires_load() {
    let execution = run("window.payload=42;throw Error('payload')", false);
    assert_eq!(execution.failed.len(), 1);
    assert_eq!(execution.effects.console.last().unwrap().text, "1:0:42");
}

/** A refused resource gets only error and can never execute its supplied source. */
#[test]
fn refused_script_fires_error_without_executing() {
    let execution = run("window.payload=42", true);
    assert_eq!(execution.effects.console.last().unwrap().text, "0:1:0");
}

/** Module acquisition pauses retain the already-created classic script's DOM identity. */
fn resume_scripts(corrupt: Option<Vec<usize>>) -> mars::protocol::EngineResult<v9::Execution> {
    let document = document::parse("<div id=out></div>").unwrap();
    let host = v9::DocumentHost {
        imports: None,
        canvas: &mars_core::canvas::Unavailable,
        languages: &[],
        fragments: &document::DomFragments,
    };
    let attributes = [("id", "payload"), ("onload", "window.loads++")]
        .into_iter()
        .map(|(name, value)| (name.into(), value.into()))
        .collect();
    let environment = v9::PageEnvironment {
        dom_level: v9::DomLevel::Extended,
        scripts: vec![
            String::new(),
            String::new(),
            "https://example.test/payload.js".into(),
        ],
        script_attributes: vec![Default::default(), Default::default(), attributes],
        ..Default::default()
    };
    let sources = ["window.loads=0;window.saved=document.getElementById('payload')".into(), String::new(), "if(window.saved!==document.currentScript||document.querySelectorAll('#payload').length!==1)throw Error('script identity');console.log('stable')".into()];
    let mut modules = v9::ModuleInput::default();
    modules.entries.push(v9::ModuleEntry {
        index: 1,
        kind: v9::EntryKind::Module,
        url: Some("https://example.test/module.js".into()),
    });
    let initial = v9::load_page(v9::PageInput {
        sources: &sources,
        source_keys: &[],
        module_keys: &[],
        document: document::script_document(&document, "parsed-resume").unwrap(),
        storage: Default::default(),
        host: &host,
        environment: &environment,
        modules: &modules,
    })
    .unwrap();
    assert!(initial.failed.is_empty());
    assert!(!initial.module_requests.is_empty());
    modules.sources.push(v9::ModuleSource {
        url: "https://example.test/module.js".into(),
        source: Some("export const ready=true".into()),
        classic: false,
    });
    let mut state = initial.state;
    if let Some(ids) = corrupt {
        let mut wire = serde_json::to_value(&state).unwrap();
        wire["page"]["parsed_scripts"] = serde_json::json!(ids);
        state = serde_json::from_value(wire).unwrap();
    }
    v9::load_modules(
        v9::PageInput {
            sources: &sources,
            source_keys: &[],
            module_keys: &[],
            document: document::script_document(&document, "parsed-resume").unwrap(),
            storage: Default::default(),
            host: &host,
            environment: &environment,
            modules: &modules,
        },
        state,
    )
}

/** Module pauses preserve script identity and do not recreate metadata or rerun earlier sources. */
#[test]
fn module_resume_reuses_script_elements_and_fires_classic_load_once() {
    let resumed = resume_scripts(None).unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", resumed.failed);
    assert_eq!(resumed.effects.console.last().unwrap().text, "stable");
}

/** Corrupt cached identities are contract failures, never unchecked indexing or arbitrary targets. */
#[test]
fn resumed_script_cache_rejects_missing_duplicate_and_unknown_identities() {
    for ids in [
        vec![],
        vec![0, 0, 0],
        vec![usize::MAX, usize::MAX - 1, usize::MAX - 2],
    ] {
        assert!(resume_scripts(Some(ids)).is_err());
    }
}
