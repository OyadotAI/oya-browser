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
/** Each element's own custom properties plus its parent scope; lookups walk ancestors. */
#[derive(Default)]
pub(super) struct Scopes {
    /// Declared values per scope, shared with the rules that declared them.
    own: Vec<BTreeMap<Rc<str>, Rc<str>>>,
    /// Parent scope per scope.
    parent: Vec<Option<usize>>,
}
impl Scopes {
    /** Adds the next scope in document order. */
    pub(super) fn push(&mut self, parent: Option<usize>, own: BTreeMap<Rc<str>, Rc<str>>) {
        self.own.push(own);
        self.parent.push(parent);
    }
    /** Scopes pushed so far. */
    pub(super) fn len(&self) -> usize {
        self.own.len()
    }
    /** Nearest declaration of `name` at or above `scope`. */
    fn lookup(&self, scope: usize, name: &str) -> Option<(usize, Rc<str>)> {
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
