/**
 * In-scope namespace bindings. Each prefix keeps a stack of its bindings, so a lookup is one
 * hash probe and a binding or an element's end pops the entries it pushed, with no scan of the
 * open elements.
 */
use crate::XML_NAMESPACE;
use std::collections::HashMap;

/** The bindings in scope, keyed by prefix; the empty key is the default namespace. */
pub struct Scopes {
    /** For each prefix, its bindings innermost last. `None` is an `xmlns=""` undeclaration. */
    bound: HashMap<String, Vec<Option<String>>>,
}

impl Scopes {
    /** A scope with only the `xml` prefix bound, which every document has. */
    pub fn new() -> Self {
        let mut bound = HashMap::new();
        bound.insert("xml".to_string(), vec![Some(XML_NAMESPACE.to_string())]);
        Self { bound }
    }

    /** Bind `prefix` to `uri` (None undeclares it) until the matching `unbind`. */
    pub fn bind(&mut self, prefix: &str, uri: Option<String>) {
        self.bound.entry(prefix.to_string()).or_default().push(uri);
    }

    /** Undo the innermost binding of `prefix`. */
    pub fn unbind(&mut self, prefix: &str) {
        if let Some(stack) = self.bound.get_mut(prefix) {
            stack.pop();
        }
    }

    /** The default namespace in scope, or None for the null namespace. */
    pub fn default_namespace(&self) -> Option<String> {
        self.prefixed("")
    }

    /** The namespace a non-empty prefix is bound to, or None when it is unbound. */
    pub fn prefixed(&self, prefix: &str) -> Option<String> {
        self.bound
            .get(prefix)
            .and_then(|stack| stack.last())
            .cloned()
            .flatten()
    }
}
