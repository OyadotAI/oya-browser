/**
 * The custom element registry: `customElements.define/get/getName/whenDefined/upgrade`, the
 * HTMLElement constructor that `class extends HTMLElement` calls through `super()`, element
 * creation for defined names, and the upgrade of elements that existed before their definition.
 */
use super::{DEFINITIONS, Definition};
use crate::v9::platform::Page;
use crate::v9::platform::dom::{self, dom_error, native_name, wrap};
use crate::v9::value::JsValue;
use crate::v9::vm::heap::Key;
use crate::v9::vm::{Call, Res, Thrown, Vm};

/** Names the specification reserves even though they contain a hyphen. */
const RESERVED: [&str; 8] = [
    "annotation-xml",
    "color-profile",
    "font-face",
    "font-face-src",
    "font-face-uri",
    "font-face-format",
    "font-face-name",
    "missing-glyph",
];
/** Marks a construction-stack entry whose element `super()` already returned. */
const CONSTRUCTED: usize = usize::MAX;

/** Whether a name is a valid custom element name. */
pub(in crate::v9) fn valid_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.contains('-')
        && !RESERVED.contains(&name)
        && name.chars().all(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || matches!(c, '-' | '.' | '_')
                || !c.is_ascii()
        })
}

/** The definition an element would run: its own name, or its `is` value for a built-in. */
pub(super) fn definition_for(page: &mut Page, node: usize) -> Option<usize> {
    if page.state.components.definitions.is_empty() {
        return None;
    }
    let view = page.view(node)?;
    let definitions = &page.state.components.definitions;
    if valid_name(&view.tag) {
        return definitions
            .iter()
            .position(|definition| definition.name == view.tag && definition.extends.is_none());
    }
    let is = view.attributes.get("is")?;
    definitions.iter().position(|definition| {
        definition.name == *is && definition.extends.as_deref() == Some(view.tag.as_str())
    })
}

/** CustomElementRegistry methods. */
pub(in crate::v9) fn registry_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match native_name(&call) {
        "define" => define(vm, &call),
        "get" => {
            let name = vm.to_text(&call.arg(0))?;
            Ok(vm
                .page
                .state
                .components
                .definitions
                .iter()
                .find(|definition| definition.name == name)
                .map_or(JsValue::Undefined, |definition| {
                    definition.constructor.clone()
                }))
        }
        "getName" => {
            let constructor = call.arg(0);
            match vm
                .page
                .state
                .components
                .definitions
                .iter()
                .find(|definition| definition.constructor == constructor)
            {
                Some(definition) => Ok(JsValue::string(&definition.name.clone())?),
                None => Ok(JsValue::Null),
            }
        }
        "whenDefined" => when_defined(vm, &call),
        "upgrade" => {
            if let Some(root) = dom::node_of(vm, &call.arg(0)) {
                super::reactions::upgrade_tree(vm, root)?;
            }
            Ok(JsValue::Undefined)
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** `customElements.define(name, constructor, {extends})`. */
fn define(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let name = vm.to_text(&call.arg(0))?;
    let constructor = call.arg(1);
    if !vm.is_constructor(&constructor) {
        return Err(vm.type_error("Failed to execute 'define' on 'CustomElementRegistry': The provided callback is not a constructor."));
    }
    if !valid_name(&name) {
        return Err(vm.dom_exception(
            "SyntaxError",
            &format!("Failed to execute 'define' on 'CustomElementRegistry': \"{name}\" is not a valid custom element name"),
        ));
    }
    let state = &vm.page.state.components;
    if state
        .definitions
        .iter()
        .any(|definition| definition.name == name)
    {
        return Err(vm.dom_exception(
            "NotSupportedError",
            &format!("Failed to execute 'define' on 'CustomElementRegistry': the name \"{name}\" has already been used with this registry"),
        ));
    }
    if state
        .definitions
        .iter()
        .any(|definition| definition.constructor == constructor)
    {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "Failed to execute 'define' on 'CustomElementRegistry': this constructor has already been used with this registry",
        ));
    }
    if state.definitions.len() >= DEFINITIONS {
        return Err(vm.range_error("Too many custom element definitions."));
    }
    let extends = extends(vm, &call.arg(2))?;
    let definition = read_definition(vm, name.clone(), extends, constructor.clone())?;
    vm.page.state.components.definitions.push(definition);
    upgrade_document(vm, &name)?;
    if let Some(promise) = vm.page.state.components.waiting.remove(&name) {
        vm.resolve_promise(promise, constructor)?;
    }
    Ok(JsValue::Undefined)
}

/** The `extends` option: a built-in tag, never another custom name. */
fn extends(vm: &mut Vm, options: &JsValue) -> Res<Option<String>> {
    if !matches!(options, JsValue::Object(_)) {
        return Ok(None);
    }
    let value = vm.get(options, &Key::from("extends"))?;
    if value == JsValue::Undefined {
        return Ok(None);
    }
    let tag = vm.to_text(&value)?.to_ascii_lowercase();
    if valid_name(&tag) {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "Failed to execute 'define' on 'CustomElementRegistry': a customized built-in element cannot extend a custom element",
        ));
    }
    Ok(Some(tag))
}

/** Read the prototype's lifecycle callbacks and the class's observed attributes once. */
fn read_definition(
    vm: &mut Vm,
    name: String,
    extends: Option<String>,
    constructor: JsValue,
) -> Res<Definition> {
    let prototype = vm.get(&constructor, &Key::prototype())?;
    if !matches!(prototype, JsValue::Object(_)) {
        return Err(vm.type_error(
            "Failed to execute 'define' on 'CustomElementRegistry': The prototype is not an object.",
        ));
    }
    let mut callbacks = Vec::with_capacity(3);
    for callback in [
        "connectedCallback",
        "disconnectedCallback",
        "attributeChangedCallback",
    ] {
        let value = vm.get(&prototype, &Key::from(callback))?;
        if value != JsValue::Undefined && !vm.is_callable(&value) {
            return Err(vm.type_error(&format!(
                "Failed to execute 'define' on 'CustomElementRegistry': The '{callback}' property of the prototype is not a function."
            )));
        }
        callbacks.push(value);
    }
    let mut observed = Vec::new();
    if callbacks[2] != JsValue::Undefined {
        let list = vm.get(&constructor, &Key::from("observedAttributes"))?;
        if list != JsValue::Undefined {
            for value in vm.iterate_to_vec(&list)? {
                observed.push(vm.to_text(&value)?.to_ascii_lowercase());
            }
        }
    }
    let attribute_changed = callbacks.pop().unwrap_or(JsValue::Undefined);
    let disconnected = callbacks.pop().unwrap_or(JsValue::Undefined);
    let connected = callbacks.pop().unwrap_or(JsValue::Undefined);
    Ok(Definition {
        name,
        extends,
        constructor,
        connected,
        disconnected,
        attribute_changed,
        observed,
    })
}

/** Upgrade every element of a new definition already in the document, in tree order. */
fn upgrade_document(vm: &mut Vm, name: &str) -> Res<()> {
    let mut candidates = Vec::new();
    for node in vm.page.all_descendants(crate::v9::platform::HTML) {
        let matches = vm.page.with_element(node, |tag, attributes| {
            tag == name || attributes.get("is").is_some_and(|is| is == name)
        });
        if matches == Some(true) {
            candidates.push(node);
        }
    }
    for node in candidates {
        if vm.page.exists(node) {
            upgrade(vm, node)?;
        }
    }
    Ok(())
}

/** `customElements.whenDefined(name)`. */
fn when_defined(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    let name = vm.to_text(&call.arg(0))?;
    if !valid_name(&name) {
        let error = match vm.dom_exception(
            "SyntaxError",
            &format!("Failed to execute 'whenDefined' on 'CustomElementRegistry': \"{name}\" is not a valid custom element name"),
        ) {
            Thrown::Value(value) => value,
            fatal => return Err(fatal),
        };
        let promise = vm.new_promise()?;
        vm.settle(promise, crate::v9::vm::data::PromiseState::Rejected(error))?;
        return Ok(JsValue::Object(promise));
    }
    let defined = vm
        .page
        .state
        .components
        .definitions
        .iter()
        .find(|definition| definition.name == name)
        .map(|definition| definition.constructor.clone());
    if let Some(constructor) = defined {
        return Ok(JsValue::Object(vm.promise_resolve(constructor)?));
    }
    if let Some(promise) = vm.page.state.components.waiting.get(&name) {
        return Ok(JsValue::Object(*promise));
    }
    let promise = vm.new_promise()?;
    vm.page.state.components.waiting.insert(name, promise);
    Ok(JsValue::Object(promise))
}

/**
 * The HTMLElement constructor behind `super()`: during an upgrade it returns the element being
 * upgraded; otherwise it makes a new element of the definition `new.target` was registered with.
 */
pub(in crate::v9) fn html_element_construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Failed to construct 'HTMLElement': Please use the 'new' operator, this DOM object constructor cannot be called as a function."));
    };
    let Some(index) = vm
        .page
        .state
        .components
        .definitions
        .iter()
        .position(|definition| definition.constructor == target)
    else {
        return Err(vm.type_error("Failed to construct 'HTMLElement': Illegal constructor"));
    };
    let proto = vm.prototype_for(
        &target,
        crate::v9::vm::realm::realm().ids.html_element_prototype,
    )?;
    let upgrading = vm
        .page
        .state
        .components
        .constructing
        .last()
        .copied()
        .filter(|node| *node != CONSTRUCTED);
    let node = match upgrading {
        Some(node) if definition_for(&mut vm.page, node) == Some(index) => {
            if let Some(last) = vm.page.state.components.constructing.last_mut() {
                *last = CONSTRUCTED;
            }
            node
        }
        _ => new_element(vm, index)?,
    };
    let wrapper = wrap(vm, node)?;
    if let Some(id) = wrapper.object() {
        vm.set_proto(id, Some(proto))?;
    }
    Ok(wrapper)
}

/** A fresh element for a definition, already custom (its constructor is running now). */
fn new_element(vm: &mut Vm, index: usize) -> Res<usize> {
    let definition = vm.page.state.components.definitions[index].clone();
    let tag = definition
        .extends
        .clone()
        .unwrap_or(definition.name.clone());
    let node = vm
        .page
        .create(&tag, None)
        .map_err(|error| dom_error(vm, error))?;
    if definition.extends.is_some()
        && let Some(record) = vm.page.state.detached.get_mut(&node)
    {
        record.attributes.insert("is".into(), definition.name);
    }
    vm.page.state.components.custom.insert(node, index);
    Ok(node)
}

/** `document.createElement(tag, {is})`: a defined name constructs synchronously. */
pub(in crate::v9) fn create_element(vm: &mut Vm, tag: &str, options: &JsValue) -> Res<JsValue> {
    let is = if matches!(options, JsValue::Object(_)) {
        let value = vm.get(options, &Key::from("is"))?;
        (value != JsValue::Undefined)
            .then(|| vm.to_text(&value))
            .transpose()?
    } else {
        None
    };
    let autonomous = vm
        .page
        .state
        .components
        .definitions
        .iter()
        .find(|definition| definition.name == tag && definition.extends.is_none())
        .map(|definition| definition.constructor.clone());
    if let Some(constructor) = autonomous {
        match vm.construct(constructor, Vec::new(), None) {
            Ok(value) if dom::node_of(vm, &value).is_some() => return Ok(value),
            Ok(_) => {}
            Err(Thrown::Value(error)) => vm.report_error(&error),
            Err(fatal) => return Err(fatal),
        }
    }
    let node = vm
        .page
        .create(tag, None)
        .map_err(|error| dom_error(vm, error))?;
    if let Some(is) = is
        && let Some(record) = vm.page.state.detached.get_mut(&node)
    {
        record.attributes.insert("is".into(), is);
        upgrade(vm, node)?;
    }
    wrap(vm, node)
}

/**
 * Upgrade one element: run its definition's constructor with the element on the construction
 * stack, then its attributeChangedCallback for each observed attribute present and, when it is
 * connected, its connectedCallback. A throwing constructor leaves the element failed.
 */
pub(super) fn upgrade(vm: &mut Vm, node: usize) -> Res<()> {
    let state = &vm.page.state.components;
    if state.custom.contains_key(&node) || state.failed.contains(&node) {
        return Ok(());
    }
    let Some(index) = definition_for(&mut vm.page, node) else {
        return Ok(());
    };
    let definition = vm.page.state.components.definitions[index].clone();
    let wrapper = wrap(vm, node)?;
    vm.pin(wrapper.clone());
    vm.page.state.components.constructing.push(node);
    let result = vm.construct(definition.constructor.clone(), Vec::new(), None);
    vm.page.state.components.constructing.pop();
    match result {
        Ok(_) => {}
        Err(Thrown::Value(error)) => {
            vm.page.state.components.failed.insert(node);
            vm.report_error(&error);
            return Ok(());
        }
        Err(fatal) => return Err(fatal),
    }
    vm.page.state.components.custom.insert(node, index);
    if vm.page.is_projected(node) && vm.page.environment.dom_level == crate::v9::DomLevel::Extended
    {
        vm.page
            .record(crate::document::ScriptChange::Defined { node })
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    let attributes = vm
        .page
        .view(node)
        .map(|view| view.attributes)
        .unwrap_or_default();
    for name in &definition.observed {
        if let Some(value) = attributes.get(name) {
            let args = vec![
                JsValue::string(name)?,
                JsValue::Null,
                JsValue::string(value)?,
            ];
            super::reactions::call_reaction(vm, &definition.attribute_changed, node, args)?;
        }
    }
    if vm.page.connected(node) {
        super::reactions::connected(vm, node)?;
    }
    Ok(())
}

/** CustomElementRegistry's own constructor is not callable by pages. */
pub(in crate::v9) fn illegal(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Err(vm.type_error("Illegal constructor"))
}
