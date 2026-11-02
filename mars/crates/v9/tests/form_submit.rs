/** Script form submission: `form.elements`, `requestSubmit()` and `submit()` plan a GET navigation. */
use crate::common;
use common::{global, load_with};
use mars_v9::DomLevel;

/** The extended-log environment, so script-built forms are connected. */
fn extended() -> mars_v9::PageEnvironment {
    mars_v9::PageEnvironment {
        dom_level: DomLevel::Extended,
        ..common::environment()
    }
}

/** A hidden GET form, as a JavaScript challenge page builds it. */
const FORM: &str = "document.body.innerHTML='<form hidden method=\"GET\" action=\"/x\"><input type=\"hidden\" name=\"solution\"><input type=\"hidden\" name=\"js_challenge\" value=\"1\"><input type=\"checkbox\" name=\"off\"><button name=\"b\">go</button></form>';\
    const f=document.forms[0];f.elements.namedItem('solution').value='abab';let fired=0;";

/** The navigation a run recorded, or "none". */
fn navigated(source: &str) -> (String, String) {
    let run = load_with(&format!("{FORM}{source}"), &extended());
    let url = run
        .effects
        .navigation
        .as_ref()
        .map_or("none".into(), |n| n.url.clone());
    (url, global(&run, "fired"))
}

/** requestSubmit fires submit (whose handler may add fields), then navigates with the entries. */
#[test]
fn request_submit_fires_submit_and_navigates_with_the_entry_list() {
    let (url, fired) = navigated(
        "f.onsubmit=e=>{fired++;e.target.appendChild(Object.assign(document.createElement('input'),{name:'q',type:'hidden',value:'a b'}));};f.requestSubmit();",
    );
    assert_eq!(
        url,
        "https://example.com/x?solution=abab&js_challenge=1&q=a+b"
    );
    assert_eq!(fired, "1");
}

/** A canceled submit event stops the submission. */
#[test]
fn canceled_submit_event_does_not_navigate() {
    let (url, _) =
        navigated("f.addEventListener('submit',e=>e.preventDefault());f.requestSubmit();");
    assert_eq!(url, "none");
}

/** submit() skips the submit event but still submits. */
#[test]
fn submit_skips_the_event() {
    let (url, fired) = navigated("f.onsubmit=()=>fired++;f.submit();");
    assert_eq!(url, "https://example.com/x?solution=abab&js_challenge=1");
    assert_eq!(fired, "0");
}

/** The named submitter joins the entries; a button that is not this form's is refused. */
#[test]
fn request_submit_checks_and_includes_the_submitter() {
    let (url, _) = navigated("f.requestSubmit(f.elements.namedItem('b'));");
    assert_eq!(url, "https://example.com/x?solution=abab&js_challenge=1&b=");
    let (url, _) =
        navigated("try{f.requestSubmit(document.createElement('button'))}catch(e){fired=e.name}");
    assert_eq!(url, "none");
}

/** POST forms stay unsupported rather than sending a body Mars does not build. */
#[test]
fn post_forms_stay_unsupported() {
    let (url, fired) = navigated(
        "f.method='post';f.setAttribute('method','post');try{f.submit()}catch(e){fired='threw'}",
    );
    assert_eq!((url.as_str(), fired.as_str()), ("none", "threw"));
}
