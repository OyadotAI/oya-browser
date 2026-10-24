/** Custom properties inherit through a scope chain; `var()` substitutes with bounded depth and size. */
use super::{
    constants::{VAR_DEPTH, VAR_OUTPUT_BYTES},
    syntax::find_top,
};
use std::{collections::BTreeMap, rc::Rc};
/** Html wrapper scope index. */
pub(super) const HTML: usize = 0;
/** Body wrapper scope index. */
pub(super) const BODY: usize = 1;
/** Node scopes start after the two wrappers. */
pub(super) const FIRST_NODE: usize = 2;
/** One `@property` registration (CSS Properties and Values API, Blink's PropertyRegistration). */
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Registration {
    /// Whether the property inherits; a non-inherited one reads only its own element's value.
    pub inherits: bool,
    /// The value used where nothing declares the property, if the rule gave one.
    pub initial: Option<Rc<str>>,
}
/** Registered custom properties by name; the first valid `@property` for a name wins. */
pub(super) type Registrations = BTreeMap<String, Registration>;
/** Each element's own custom properties plus its parent scope; lookups walk ancestors. */
#[derive(Default)]
pub(super) struct Scopes {
    /// Declared values per scope, shared with the rules that declared them.
    own: Vec<BTreeMap<Rc<str>, Rc<str>>>,
    /// Parent scope per scope.
    parent: Vec<Option<usize>>,
    /// `@property` registrations from the page's sheets.
    registered: Rc<Registrations>,
}
impl Scopes {
    /** Empty scopes that honour the sheets' `@property` registrations. */
    pub(super) fn registered(registered: Rc<Registrations>) -> Self {
        Self {
            registered,
            ..Self::default()
        }
    }
    /** Adds the next scope in document order. */
    pub(super) fn push(&mut self, parent: Option<usize>, own: BTreeMap<Rc<str>, Rc<str>>) {
        self.own.push(own);
        self.parent.push(parent);
    }
    /** Drops the last scope: a pseudo-element's, which lives only while its box computes. */
    pub(super) fn pop(&mut self) {
        self.own.pop();
        self.parent.pop();
    }
    /** Scopes pushed so far. */
    pub(super) fn len(&self) -> usize {
        self.own.len()
    }
    /**
     * The computed value of `name` at `scope` and the scope it was declared in: the nearest
     * declaration at or above it, or only its own for a non-inherited registered property, and
     * a registered property's initial value (at the root, where it has no references) otherwise.
     */
    fn lookup(&self, scope: usize, name: &str) -> Option<(usize, Rc<str>)> {
        let Some(registration) = self.registered.get(name) else {
            return self.declared(scope, name);
        };
        let found = match registration.inherits {
            true => self.declared(scope, name),
            false => self
                .own
                .get(scope)?
                .get(name)
                .map(|value| (scope, value.clone())),
        };
        let initial = || Some((HTML, registration.initial.clone()?));
        match found {
            Some((_, value)) if value.trim().eq_ignore_ascii_case("initial") => initial(),
            None => initial(),
            found => found,
        }
    }
    /** Nearest declaration of `name` at or above `scope`. */
    fn declared(&self, scope: usize, name: &str) -> Option<(usize, Rc<str>)> {
        let mut current = Some(scope);
        let mut steps = 0;
        while let Some(at) = current {
            if let Some(value) = self.own.get(at)?.get(name) {
                return Some((at, value.clone()));
            }
            current = self.parent.get(at).copied().flatten();
            steps += 1;
            if steps > self.parent.len() {
                return None;
            }
        }
        None
    }
    /** Substitutes every `var()`; None is invalid at computed-value time. */
    pub(super) fn substitute(&self, scope: usize, value: &str) -> Option<String> {
        self.expand(scope, value, 0)
    }
    /** One substitution level. */
    fn expand(&self, scope: usize, value: &str, depth: usize) -> Option<String> {
        if depth > VAR_DEPTH {
            return None;
        }
        let mut out = String::new();
        let mut rest = value;
        while let Some(at) = rest.to_ascii_lowercase().find("var(") {
            out.push_str(&rest[..at]);
            let inner_start = at + "var(".len();
            let close = find_top(rest, inner_start, b")");
            let inner = rest.get(inner_start..close)?;
            out.push_str(&self.reference(scope, inner, depth)?);
            rest = rest.get(close + 1..).unwrap_or("");
            if out.len() > VAR_OUTPUT_BYTES {
                return None;
            }
        }
        out.push_str(rest);
        (out.len() <= VAR_OUTPUT_BYTES).then_some(out)
    }
    /** `--name[, fallback]`: the declared value expanded where it was declared, else the fallback. */
    fn reference(&self, scope: usize, inner: &str, depth: usize) -> Option<String> {
        let comma = find_top(inner, 0, b",");
        let name = inner[..comma].trim();
        let fallback = inner.get(comma + 1..);
        let declared = self
            .lookup(scope, name)
            .filter(|(_, value)| !["initial", ""].contains(&value.trim()))
            .and_then(|(at, value)| self.expand(at, &value, depth + 1));
        declared.or_else(|| self.expand(scope, fallback?.trim(), depth + 1))
    }
}
