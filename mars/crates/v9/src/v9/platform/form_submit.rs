/**
 * Script-driven form submission: `form.elements`, `form.submit()` and `form.requestSubmit()`,
 * following HTML's "submit a form" steps for GET forms. The entry list becomes the action URL's
 * query and the app performs the navigation, as it does for `location.assign`. POST bodies,
 * other targets and other encodings stay unsupported, so nothing is sent that Mars cannot send.
 */
use super::dom::{DOCUMENT, search, wrap};
use super::nodes::NodeView;
use super::{NavigationRequest, net};
use crate::v9::value::JsValue;
use crate::v9::vm::{Call, Res, Vm};

/** HTML's listed elements, the members of `form.elements`. */
const LISTED: [&str; 7] = [
    "button", "fieldset", "input", "object", "output", "select", "textarea",
];

/** HTML's submittable elements, the ones that can add entries. */
const SUBMITTABLE: [&str; 5] = ["button", "input", "object", "select", "textarea"];

/// The form owner: the form named by a `form` attribute, else the nearest form ancestor.
fn owner(vm: &mut Vm, node: usize) -> Option<usize> {
    let named = vm
        .page
        .view(node)
        .and_then(|v| v.attributes.get("form").cloned());
    if let Some(id) = named {
        let found = search(vm, DOCUMENT, |page, n| {
            page.tag(n) == "form"
                && page
                    .view(n)
                    .is_some_and(|v| v.attributes.get("id") == Some(&id))
        });
        return found.first().copied();
    }
    vm.page
        .ancestors(node)
        .into_iter()
        .find(|a| vm.page.tag(*a) == "form")
}

/// The form's associated elements of `kinds`, in tree order. Image buttons are never listed.
fn associated(vm: &mut Vm, form: usize, kinds: &[&str]) -> Vec<usize> {
    let candidates = search(vm, DOCUMENT, |page, n| {
        let tag = page.tag(n);
        kinds.contains(&tag.as_str())
            && !(tag == "input" && input_type(page.view(n).as_ref()) == "image")
    });
    candidates
        .into_iter()
        .filter(|n| owner(vm, *n) == Some(form))
        .collect()
}

/// `form.elements`: a snapshot of the listed elements the form owns.
pub(super) fn controls(vm: &mut Vm, form: usize) -> Vec<usize> {
    associated(vm, form, &LISTED)
}

/** An input's type, lowercased, defaulting to text. */
fn input_type(view: Option<&NodeView>) -> String {
    view.and_then(|v| v.attributes.get("type"))
        .map_or_else(|| "text".into(), |t| t.to_ascii_lowercase())
}

/** Whether a button or input is a submit button. */
fn is_submit_button(view: &NodeView) -> bool {
    let kind = view.attributes.get("type").map(|t| t.to_ascii_lowercase());
    match view.tag.as_str() {
        "button" => kind.is_none_or(|t| t != "button" && t != "reset"),
        "input" => matches!(kind.as_deref(), Some("submit" | "image")),
        _ => false,
    }
}

/// `submit()` (no events, no validation) and `requestSubmit(submitter)` (checks the submitter,
/// fires a cancelable `submit`, then submits), as HTMLFormElement defines them.
pub(super) fn submit(vm: &mut Vm, form: usize, call: &Call, request: bool) -> Res<JsValue> {
    let mut submitter = None;
    if request && !call.arg(0).nullish() {
        let node = super::dom::node_of(vm, &call.arg(0));
        let Some(node) = node.filter(|n| vm.page.view(*n).is_some_and(|v| is_submit_button(&v)))
        else {
            return Err(vm.type_error("Failed to execute 'requestSubmit' on 'HTMLFormElement': The specified element is not a submit button."));
        };
        if owner(vm, node) != Some(form) {
            return Err(vm.dom_exception("NotFoundError", "Failed to execute 'requestSubmit' on 'HTMLFormElement': The specified element is not owned by this form element."));
        }
        submitter = Some(node);
    }
    if !vm.page.connected(form) {
        return Ok(JsValue::Undefined);
    }
    if request && !super::events::fire(vm, super::Target::Node(form), "submit", true, true)? {
        return Ok(JsValue::Undefined);
    }
    plan(vm, form, submitter)?;
    Ok(JsValue::Undefined)
}

/// Build the GET navigation: the action URL (the document's when empty) with the
/// urlencoded entry list as its query.
fn plan(vm: &mut Vm, form: usize, submitter: Option<usize>) -> Res<()> {
    let view = vm.page.view(form).expect("form view");
    let attr = |name: &str| {
        view.attributes
            .get(name)
            .map(|s| s.trim().to_ascii_lowercase())
    };
    if attr("method").is_some_and(|m| m != "get")
        || attr("target").is_some_and(|t| !t.is_empty() && t != "_self")
    {
        return Err(vm.unsupported_error("form submission from script other than GET to _self"));
    }
    let action = view
        .attributes
        .get("action")
        .filter(|a| !a.trim().is_empty());
    let href = vm.page.state.href.clone();
    let Some(mut url) =
        net::parse_url(&net::resolve(vm, action.unwrap_or(&href)).unwrap_or_default())
    else {
        return Ok(());
    };
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in entries(vm, form, submitter) {
        query.append_pair(
            &name,
            &value
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .replace('\n', "\r\n"),
        );
    }
    if matches!(url.scheme(), "http" | "https") {
        url.set_query(Some(&query.finish()));
    }
    vm.page.effects.navigation = Some(NavigationRequest {
        kind: "assign".into(),
        url: url.to_string(),
        delta: 0,
    });
    Ok(())
}

/// HTML's "constructing the entry list", for the string values a GET query carries.
fn entries(vm: &mut Vm, form: usize, submitter: Option<usize>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for node in associated(vm, form, &SUBMITTABLE) {
        let Some(view) = vm.page.view(node) else {
            continue;
        };
        let kind = input_type(Some(&view));
        let disabled = view.attributes.contains_key("disabled");
        let unchecked = view.tag == "input"
            && matches!(kind.as_str(), "checkbox" | "radio")
            && !view.attributes.contains_key("checked");
        let idle_button = is_submit_button(&view) && Some(node) != submitter
            || view.tag == "button" && !is_submit_button(&view)
            || view.tag == "input" && matches!(kind.as_str(), "button" | "reset");
        let Some(name) = view
            .attributes
            .get("name")
            .filter(|n| !n.is_empty())
            .cloned()
        else {
            continue;
        };
        if disabled || unchecked || idle_button || view.tag == "object" {
            continue;
        }
        for value in values(vm, node, &view, &kind) {
            out.push((name.clone(), value));
        }
    }
    out
}

/** The values one control contributes. */
fn values(vm: &mut Vm, node: usize, view: &NodeView, kind: &str) -> Vec<String> {
    let attribute = view.attributes.get("value").cloned();
    match view.tag.as_str() {
        "select" => vm
            .page
            .descendants(node)
            .into_iter()
            .filter_map(|o| vm.page.view(o))
            .filter(|o| o.tag == "option" && o.attributes.contains_key("selected"))
            .map(|o| {
                o.attributes
                    .get("value")
                    .cloned()
                    .unwrap_or_else(|| o.text.clone().unwrap_or_default())
            })
            .collect(),
        "textarea" => vec![view.value.clone().unwrap_or_else(|| text(vm, node))],
        _ if matches!(kind, "checkbox" | "radio") => vec![attribute.unwrap_or_else(|| "on".into())],
        _ if kind == "file" => vec![String::new()],
        _ => vec![view.value.clone().or(attribute).unwrap_or_default()],
    }
}

/** A textarea's default value: its child text. */
fn text(vm: &mut Vm, node: usize) -> String {
    let children = vm.page.children(node);
    children
        .into_iter()
        .filter_map(|c| vm.page.view(c))
        .filter_map(|v| v.text)
        .collect()
}

/** Wrap a node list entry; kept here so `elements` and `namedItem` share one lookup. */
pub(super) fn named(vm: &mut Vm, nodes: &[usize], key: &str) -> Res<JsValue> {
    // An empty name never names an element, even one whose id or name is also empty.
    if key.is_empty() {
        return Ok(JsValue::Null);
    }
    let found = nodes.iter().copied().find(|n| {
        vm.page.view(*n).is_some_and(|v| {
            v.attributes.get("id").map(String::as_str) == Some(key)
                || v.attributes.get("name").map(String::as_str) == Some(key)
        })
    });
    match found {
        Some(node) => wrap(vm, node),
        None => Ok(JsValue::Null),
    }
}
