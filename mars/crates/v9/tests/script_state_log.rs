/** Page state the document engine styles from: writes to `<html>` and `<body>` attributes and
focus moves travel in the extended change log, and element ids answer bare global names. */
mod common;

use common::{environment, failure, global, load_with};
use mars_v9::{DomLevel, Execution, document::ScriptChange};

/** Run `source` with the extended change log on. */
fn extended(source: &str) -> Execution {
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    let execution = load_with(source, &environment);
    assert_eq!(failure(&execution), None);
    execution
}

#[test]
fn html_class_writes_are_logged_for_the_document_engine() {
    let execution = extended(
        "document.documentElement.className = 'reftest-wait';
         document.documentElement.classList.remove('reftest-wait');",
    );
    let logged: Vec<_> = execution
        .script_changes
        .iter()
        .filter_map(|change| match change {
            ScriptChange::RootAttribute { body, name, value } => {
                Some((*body, name.clone(), value.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        logged.last(),
        Some(&(false, "class".into(), Some(String::new())))
    );
}

#[test]
fn body_attribute_writes_are_logged_as_body() {
    let execution = extended("document.body.setAttribute('data-x', '1');");
    assert!(execution.script_changes.iter().any(|change| matches!(
        change,
        ScriptChange::RootAttribute { body: true, name, .. } if name == "data-x"
    )));
}

#[test]
fn focus_and_blur_are_logged() {
    let execution = extended("const b = document.getElementById('go'); b.focus(); b.blur();");
    let focus: Vec<_> = execution
        .script_changes
        .iter()
        .filter_map(|change| match change {
            ScriptChange::Focus { node } => Some(node.is_some()),
            _ => None,
        })
        .collect();
    assert_eq!(focus, vec![true, false]);
}

#[test]
fn an_element_id_answers_a_bare_global_name() {
    let execution = extended("const r = [go.id, typeof main, typeof nothing_here].join('|');");
    assert_eq!(global(&execution, "r"), "go|object|undefined");
}

#[test]
fn an_unknown_bare_name_still_throws() {
    let mut environment = environment();
    environment.dom_level = DomLevel::Extended;
    let execution = load_with("nothing_here.x;", &environment);
    assert!(failure(&execution).is_some_and(|message| message.contains("not defined")));
}
