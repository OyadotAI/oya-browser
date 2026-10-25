//! Module conformance rules ported for test262: early errors of module code, indirect exports
//! that must resolve when linking, re-exported namespaces, and dynamic import of a module with a
//! syntax error. Hermetic: modules are served from memory, round after round, as the app does.
mod common;

use common::{environment, host, page};
use mars_v9::{
    EntryKind, Execution, ModuleEntry, ModuleInput, ModuleSource, PageInput, StorageState,
};
use std::collections::BTreeMap;

/** Where test modules live. */
const BASE: &str = "https://example.com/app/";

/** Load `main.js` as a module script, serving `files` by path; console lines and failures. */
fn load(files: &[(&str, &str)]) -> (Vec<String>, Vec<String>) {
    let files: BTreeMap<String, String> = files
        .iter()
        .map(|(path, source)| (format!("{BASE}{path}"), source.to_string()))
        .collect();
    let sources = vec![String::new()];
    let mut input = ModuleInput::default();
    input.entries.push(ModuleEntry {
        index: 0,
        kind: EntryKind::Module,
        url: Some(format!("{BASE}main.js")),
    });
    let host = host();
    let environment = environment();
    let request = |input: &ModuleInput, state: Option<mars_v9::VmState>| -> Execution {
        let page_input = PageInput {
            sources: &sources,
            document: page(),
            storage: StorageState::default(),
            host: &host,
            environment: &environment,
            modules: input,
        };
        match state {
            None => mars_v9::load_page(page_input).expect("page loads"),
            Some(state) => mars_v9::load_modules(page_input, state).expect("modules load"),
        }
    };
    let mut execution = request(&input, None);
    let (mut console, mut failures) = (Vec::new(), Vec::new());
    for _ in 0..8 {
        console.extend(common::console(&execution));
        failures.extend(execution.failed.iter().map(|f| f.error.message.clone()));
        if execution.module_requests.is_empty() {
            break;
        }
        for request in &execution.module_requests {
            input.sources.push(ModuleSource {
                url: request.url.clone(),
                source: files.get(&request.url).cloned(),
                classic: false,
            });
        }
        execution = request(&input, Some(execution.state.clone()));
    }
    (console, failures)
}

/** The first failure of a lone module, which must be a syntax error. */
fn syntax_error(source: &str) -> String {
    let (console, failures) = load(&[("main.js", source)]);
    assert!(console.is_empty(), "module ran: {console:?}");
    failures.into_iter().next().expect("module fails")
}

#[test]
fn module_top_level_functions_are_lexical_and_may_not_be_redeclared() {
    syntax_error("console.log('ran'); function f() {} function f() {}");
    syntax_error("console.log('ran'); var f; function f() {}");
}

#[test]
fn an_import_binding_may_not_be_redeclared_or_named_eval() {
    syntax_error("import { a } from './lib.js'; var a; console.log('ran');");
    syntax_error("import { a as eval } from './lib.js'; console.log('ran');");
    syntax_error("import arguments from './lib.js'; console.log('ran');");
}

#[test]
fn a_nested_label_may_not_reuse_its_enclosing_label() {
    syntax_error("console.log('ran'); label: { label: 0; }");
}

#[test]
fn a_string_export_name_needs_from() {
    syntax_error("var foo; export { 'foo' }; console.log('ran');");
}

#[test]
fn an_indirect_export_that_does_not_resolve_fails_linking() {
    let (console, failures) = load(&[
        (
            "main.js",
            "export { x } from './empty.js'; console.log('ran');",
        ),
        ("empty.js", ""),
    ]);
    assert!(console.is_empty(), "{console:?}");
    assert!(failures[0].contains("cannot be resolved"), "{failures:?}");
    let (console, failures) = load(&[
        (
            "main.js",
            "export { x } from './loop.js'; console.log('ran');",
        ),
        ("loop.js", "export { x } from './main.js';"),
    ]);
    assert!(console.is_empty() && !failures.is_empty(), "{console:?}");
}

#[test]
fn a_reexported_namespace_import_is_not_ambiguous_across_star_exports() {
    let (console, failures) = load(&[
        (
            "main.js",
            "export * from './one.js'; export * from './two.js';\n\
             import { ns } from './main.js'; console.log(typeof ns, ns.v);",
        ),
        ("one.js", "import * as ns from './lib.js'; export { ns };"),
        ("two.js", "import * as ns from './lib.js'; export { ns };"),
        ("lib.js", "export const v = 7;"),
    ]);
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(console, ["object 7"]);
}

#[test]
fn dynamic_import_of_a_module_with_a_syntax_error_rejects_with_syntax_error() {
    let (console, failures) = load(&[
        (
            "main.js",
            "import('./bad.js').then(() => console.log('loaded'), e => console.log(e.name));",
        ),
        ("bad.js", "export {"),
    ]);
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(console, ["SyntaxError"]);
}

#[test]
fn an_export_name_with_a_lone_surrogate_is_a_syntax_error() {
    let message = syntax_error("var a; export { a as '\\uD83D' }; console.log('ran');");
    assert!(message.contains("well-formed"), "{message}");
}
