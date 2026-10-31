//! JavaScript modules: syntax, linking, cycles, live bindings, dynamic import, import maps,
//! top-level await, isolated failures and bounds, through the page entry points the app uses.
use crate::common;

use common::{environment, host, page, round_trip};
use mars_v9::{
    DocumentHost, EntryKind, Execution, ModuleEntry, ModuleInput, ModuleSource, PageEnvironment,
    PageInput, StorageState,
};
use std::collections::BTreeMap;

/** Where test modules live. */
const BASE: &str = "https://example.com/app/";

/** Private slots on module namespaces remain writable while exported bindings stay read-only. */
#[test]
fn namespace_private_fields_are_distinct_from_read_only_exports() {
    let loaded = load(
        vec![Source::Inline(
            "import * as ns from './dep.mjs';class Base{constructor(value){return value}}class C extends Base{#x=1;read(){return this.#x}write(v){this.#x=v}}new C(ns);C.prototype.write.call(ns,4);let rejected=false;try{ns.value=2}catch(e){rejected=e instanceof TypeError}console.log(C.prototype.read.call(ns),ns.value,rejected,Reflect.ownKeys(ns).length);",
        )],
        &[("dep.mjs", "export const value=7;")],
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.reasons);
    assert_eq!(loaded.console, ["4 7 true 2"]);
}

/** One page source: a classic script, an inline module, an external module or an import map. */
enum Source {
    Classic(&'static str),
    Inline(&'static str),
    External(&'static str),
    Async(&'static str),
    Map(&'static str),
}

/** What a page load produced across its module rounds. */
struct Loaded {
    console: Vec<String>,
    failed: Vec<usize>,
    reasons: Vec<String>,
    rounds: usize,
    requested: Vec<String>,
    last: Execution,
}

/** Load a page, serving module requests from `files` (by URL relative to BASE) round after
round, as the app does, with the state round-tripped through JSON between requests. */
fn load(page_sources: Vec<Source>, files: &[(&str, &str)]) -> Loaded {
    let files: BTreeMap<String, String> = files
        .iter()
        .map(|(path, source)| (format!("{BASE}{path}"), source.to_string()))
        .collect();
    let mut sources = Vec::new();
    let mut input = ModuleInput::default();
    for (index, source) in page_sources.iter().enumerate() {
        let (text, kind, url) = match source {
            Source::Classic(text) => (text.to_string(), None, None),
            Source::Inline(text) => (text.to_string(), Some(EntryKind::Module), None),
            Source::External(path) => (String::new(), Some(EntryKind::Module), Some(*path)),
            Source::Async(path) => (String::new(), Some(EntryKind::AsyncModule), Some(*path)),
            Source::Map(text) => (text.to_string(), Some(EntryKind::ImportMap), None),
        };
        sources.push(text);
        if let Some(kind) = kind {
            input.entries.push(ModuleEntry {
                index,
                kind,
                url: url.map(|path| format!("{BASE}{path}")),
            });
        }
    }
    let host = host();
    let environment = environment();
    let mut execution =
        mars_v9::load_page(page_input(&sources, &host, &environment, &input)).expect("page loads");
    let mut loaded = Loaded {
        console: Vec::new(),
        failed: Vec::new(),
        reasons: Vec::new(),
        rounds: 0,
        requested: Vec::new(),
        last: execution.clone(),
    };
    loop {
        collect(&mut loaded, &execution);
        if execution.module_requests.is_empty() || loaded.rounds > 8 {
            break;
        }
        loaded.rounds += 1;
        for request in &execution.module_requests {
            loaded.requested.push(request.url.clone());
            input.sources.push(ModuleSource {
                url: request.url.clone(),
                source: files.get(&request.url).cloned(),
                classic: false,
            });
        }
        let state = round_trip(execution.state.clone());
        execution = mars_v9::load_modules(page_input(&sources, &host, &environment, &input), state)
            .expect("modules load");
    }
    loaded.last = execution;
    loaded
}

/** The input for one request against the test page. */
fn page_input<'a>(
    sources: &'a [String],
    host: &'a DocumentHost<'a>,
    environment: &'a PageEnvironment,
    modules: &'a ModuleInput,
) -> PageInput<'a> {
    PageInput {
        sources,
        source_keys: &[],
        document: page(),
        storage: StorageState::default(),
        host,
        environment,
        modules,
    }
}

/** Gather one execution's console output and failures. */
fn collect(loaded: &mut Loaded, execution: &Execution) {
    loaded.console.extend(
        execution
            .effects
            .console
            .iter()
            .map(|entry| entry.text.clone()),
    );
    for failure in &execution.failed {
        loaded.failed.push(failure.index);
        loaded.reasons.push(failure.error.message.clone());
    }
}

#[test]
fn every_import_and_export_form_links() {
    let loaded = load(
        vec![Source::External("main.js")],
        &[
            (
                "main.js",
                "import d, { a, b as bee, \"str name\" as s } from './lib.js';\n\
                 import * as ns from './lib.js';\n\
                 import { again, star, nested } from './reexport.js';\n\
                 import './side.js';\n\
                 console.log(d(), a, bee, s, ns.a, Object.keys(ns).join(','), again, star, nested.a);\n\
                 console.log(Object.prototype.toString.call(ns), typeof ns.missing);",
            ),
            (
                "lib.js",
                "export default function () { return 'default'; }\n\
                 export const a = 1; export let b = 2;\n\
                 const c = 3; export { c as \"str name\" };\n\
                 export class K {} export function f() {}",
            ),
            (
                "reexport.js",
                "export { a as again } from './lib.js'; export * from './star.js';\n\
                 export * as nested from './lib.js';",
            ),
            (
                "star.js",
                "export const star = 'star'; export default 'hidden';",
            ),
            ("side.js", "console.log('side effect');"),
        ],
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.reasons);
    assert_eq!(
        loaded.console,
        [
            "side effect",
            "default 1 2 3 1 K,a,b,default,f,str name 1 star 1",
            "[object Module] undefined"
        ]
    );
}

#[test]
fn modules_load_in_rounds_and_dependencies_run_first() {
    let loaded = load(
        vec![
            Source::Classic("console.log('classic')"),
            Source::External("a.js"),
            Source::Classic("console.log('deferred classic after the module')"),
        ],
        &[
            ("a.js", "import './b.js'; console.log('a');"),
            ("b.js", "import './c.js'; console.log('b');"),
            ("c.js", "console.log('c');"),
        ],
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.reasons);
    assert_eq!(loaded.rounds, 3);
    assert_eq!(
        loaded.console,
        [
            "classic",
            "c",
            "b",
            "a",
            "deferred classic after the module"
        ]
    );
}

#[test]
fn bindings_are_live_and_read_only_to_importers() {
    let loaded = load(
        vec![Source::Inline(
            "import { count, increment } from './counter.js';\n\
             console.log(count); increment(); console.log(count);\n\
             try { count = 5; } catch (error) { console.log(error.name); }",
        )],
        &[(
            "counter.js",
            "export let count = 0; export function increment() { count++; }",
        )],
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.reasons);
    assert_eq!(loaded.console, ["0", "1", "TypeError"]);
}

#[test]
fn cycles_see_hoisted_functions_and_dead_zones() {
    let loaded = load(
        vec![Source::External("a.js")],
        &[
            (
                "a.js",
                "import { fromB } from './b.js';\n\
                 export function hoisted() { return 'hoisted'; }\n\
                 export const late = 'late';\n\
                 console.log('a sees', fromB);",
            ),
            (
                "b.js",
                "import { hoisted, late } from './a.js';\n\
                 export const fromB = hoisted();\n\
                 try { late; } catch (error) { console.log(error.name); }\n\
                 console.log('b ran');",
            ),
        ],
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.reasons);
    assert_eq!(
        loaded.console,
        ["ReferenceError", "b ran", "a sees hoisted"]
    );
}

#[test]
fn dynamic_import_loads_later_from_modules_and_classic_scripts() {
    let loaded = load(
        vec![
            Source::Classic(
                "import('./lazy.js').then(ns => console.log('classic got', ns.value));",
            ),
            Source::Inline(
                "const ns = await import('./lazy.js');\n\
                 console.log('module got', ns.value, ns === await import('./lazy.js'));\n\
                 import('./missing.js').catch(error => console.log(error.name));\n\
                 import('bare').catch(error => console.log(error.name));",
            ),
        ],
        &[("lazy.js", "export const value = 42;")],
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.reasons);
    assert!(loaded.console.contains(&"classic got 42".to_string()));
    assert!(loaded.console.contains(&"module got 42 true".to_string()));
    assert!(
        loaded.console.contains(&"Error".to_string()),
        "{:?}",
        loaded.console
    );
    assert!(
        loaded.console.contains(&"TypeError".to_string()),
        "{:?}",
        loaded.console
    );
    assert_eq!(
        loaded.requested,
        [format!("{BASE}lazy.js"), format!("{BASE}missing.js")]
    );
}

#[test]
fn import_maps_resolve_bare_and_prefix_specifiers() {
    let loaded = load(
        vec![
            Source::Map(
                r#"{"imports":{"react":"./vendor/react.js","pkg/":"./vendor/pkg/"},
                    "scopes":{"./vendor/":{"react":"./vendor/scoped-react.js"}}}"#,
            ),
            Source::Inline(
                "import React from 'react'; import { util } from 'pkg/util.js';\n\
                 console.log(React, util, import.meta.url, import.meta.resolve('react'));",
            ),
        ],
        &[
            ("vendor/react.js", "export default 'react';"),
            (
                "vendor/pkg/util.js",
                "import scoped from 'react'; export const util = 'util+' + scoped;",
            ),
            ("vendor/scoped-react.js", "export default 'scoped';"),
        ],
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.reasons);
    assert_eq!(
        loaded.console,
        [format!(
            "react util+scoped https://example.com/app/page?x=1#top {BASE}vendor/react.js"
        )]
    );
}

#[test]
fn top_level_await_holds_dependents_until_it_settles() {
    let loaded = load(
        vec![Source::External("main.js")],
        &[
            (
                "main.js",
                "import { ready } from './slow.js'; console.log('main', ready);",
            ),
            (
                "slow.js",
                "console.log('slow starts');\n\
                 export const ready = await Promise.resolve('ready');\n\
                 console.log('slow ends');",
            ),
        ],
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.reasons);
    assert_eq!(loaded.console, ["slow starts", "slow ends", "main ready"]);
}

#[test]
fn a_failing_graph_fails_alone() {
    let loaded = load(
        vec![
            Source::External("broken.js"),
            Source::External("missing-export.js"),
            Source::External("unloadable.js"),
            Source::Inline("syntax error here (;"),
            Source::Inline("import { shared } from './shared.js'; console.log('good', shared);"),
            Source::Classic("console.log('classic still runs', typeof leak)"),
        ],
        &[
            (
                "broken.js",
                "import './shared.js'; import './throws.js'; console.log('never');",
            ),
            ("throws.js", "globalThis.leak = 1; throw new Error('boom');"),
            ("shared.js", "export const shared = 'shared';"),
            ("missing-export.js", "import { nope } from './shared.js';"),
            ("unloadable.js", "import './404.js';"),
        ],
    );
    assert_eq!(loaded.failed, [0, 1, 2, 3], "{:?}", loaded.reasons);
    assert!(loaded.reasons[0].contains("boom"), "{:?}", loaded.reasons);
    assert!(loaded.reasons[1].contains("nope"), "{:?}", loaded.reasons);
    assert!(loaded.reasons[2].contains("404.js"), "{:?}", loaded.reasons);
    assert_eq!(
        loaded.console,
        ["good shared", "classic still runs undefined"]
    );
}

#[test]
fn async_module_scripts_do_not_block_the_load() {
    let loaded = load(
        vec![
            Source::Async("async.js"),
            Source::Inline("console.log('deferred module');"),
            Source::Classic("console.log('classic');"),
        ],
        &[("async.js", "console.log('async module');")],
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.reasons);
    assert_eq!(
        loaded.console,
        ["deferred module", "classic", "async module"]
    );
}

#[test]
fn module_closures_survive_later_requests() {
    let loaded = load(
        vec![Source::Inline(
            "import { label } from './label.js';\n\
             setTimeout(() => console.log('timer', label, typeof window.later), 10);\n\
             export const unused = 1;",
        )],
        &[("label.js", "export const label = 'kept';")],
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.reasons);
    let mut input = ModuleInput::default();
    input.entries.push(ModuleEntry {
        index: 0,
        kind: EntryKind::Module,
        url: None,
    });
    input.sources.push(ModuleSource {
        url: format!("{BASE}label.js"),
        source: Some("export const label = 'kept';".into()),
        classic: false,
    });
    let sources = vec![
        "import { label } from './label.js';\n\
             setTimeout(() => console.log('timer', label, typeof window.later), 10);\n\
             export const unused = 1;"
            .to_string(),
    ];
    let host = host();
    let environment = environment();
    let execution = mars_v9::run_timers(
        PageInput {
            sources: &sources,
            source_keys: &[],
            document: page(),
            storage: StorageState::default(),
            host: &host,
            environment: &environment,
            modules: &input,
        },
        round_trip(loaded.last.state),
        50.0,
        8,
    )
    .expect("timers run");
    let console: Vec<&str> = execution
        .effects
        .console
        .iter()
        .map(|entry| entry.text.as_str())
        .collect();
    assert_eq!(console, ["timer kept undefined"]);
}

#[test]
fn module_bounds_are_enforced() {
    let host = host();
    let environment = environment();
    let sources = vec![String::new()];
    let mut input = ModuleInput::default();
    for index in 0..=mars_v9::constants::MODULES {
        input.sources.push(ModuleSource {
            url: format!("{BASE}{index}.js"),
            source: Some(String::new()),
            classic: false,
        });
    }
    let too_many = mars_v9::load_page(PageInput {
        sources: &sources,
        source_keys: &[],
        document: page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: &input,
    });
    assert_eq!(too_many.expect_err("bounded").code, "limit_exceeded");
    input.sources.truncate(2);
    input.sources[0].source = Some(" ".repeat(mars_v9::constants::SCRIPT_BYTES + 1));
    let too_big = mars_v9::load_page(PageInput {
        sources: &sources,
        source_keys: &[],
        document: page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: &input,
    });
    assert_eq!(too_big.expect_err("bounded").code, "limit_exceeded");
}

#[test]
fn module_syntax_errors_and_misplaced_declarations_fail_the_module() {
    let loaded = load(
        vec![
            Source::Inline("export { undeclared };"),
            Source::Inline("export const twice = 1; export { twice };"),
            Source::Inline("{ import './x.js'; }"),
            Source::Inline("return 1;"),
            Source::Classic("import './x.js';"),
            Source::Inline("console.log(this === undefined, typeof import.meta);"),
        ],
        &[],
    );
    assert_eq!(loaded.failed, [0, 1, 2, 3, 4], "{:?}", loaded.reasons);
    assert_eq!(loaded.console, ["true object"]);
}

/** A shared failing dependency keeps its named native-constructor error for each importer. */
#[test]
fn illegal_constructor_dependency_failures_name_the_actual_interface() {
    let loaded = load(
        vec![
            Source::Inline("import './dependency.mjs';"),
            Source::Inline("import './dependency.mjs';"),
        ],
        &[("dependency.mjs", "new CSSStyleSheet();")],
    );
    assert_eq!(loaded.failed, [0, 1]);
    assert_eq!(loaded.reasons.len(), 2);
    assert!(
        loaded
            .reasons
            .iter()
            .all(|reason| reason
                .contains("Failed to construct 'CSSStyleSheet': Illegal constructor")),
        "{:?}",
        loaded.reasons
    );
}
