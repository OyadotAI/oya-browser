/**
 * A page's sheets parsed once and kept for its next cascade. Every restyle of a page (after a
 * script step, a click, a resize) hands Eye the same sheet texts, and parsing github.com's
 * 5.7 MB of CSS took a third of each restyle; the parsed rules, their buckets and keyframes
 * depend only on the texts and the render context, so the last few parses stay resident in this
 * thread. A page whose scripts insert a sheet sends its old list plus one: the parser's state
 * after the old list is kept too, and only the new sheets parse. Scoped shadow sheets are
 * rewritten per document, so they parse on top of the kept state on every cascade.
 */
use super::{
    FontFace,
    animation::Keyframes,
    bucket::Buckets,
    font_face::FontFaces,
    layer::Layers,
    sheet::{Budget, Rule, parse_sheet},
    vars::Registrations,
};
use crate::{
    constants::{STYLE_BYTES, STYLE_SHEETS},
    document::RenderContext,
    protocol::EngineResult,
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

/** Parses one thread keeps: the page open and the one just left (or a tab switch). */
const PARSED_SHEETS: usize = 2;

/** Everything a cascade reads of its sheets. */
pub(super) struct Parsed {
    /// Every parsed rule in cascade order.
    pub rules: Vec<Rc<Rule>>,
    /// Rules bucketed by subject key.
    pub buckets: Buckets,
    /// `@keyframes` rules animations rest on.
    pub keyframes: Keyframes,
    /// `@font-face` rules in source order.
    pub font_faces: Vec<FontFace>,
    /// Some rule styles a `::before` or `::after` box.
    pub pseudo_rules: bool,
    /// Some rule sits inside an `@container` query.
    pub container_rules: bool,
    /// A hiding rule could not be kept: no node may be agent-actionable.
    pub untrusted: bool,
    /// The parse's value-free notes, replayed into each document that uses it.
    pub diagnostics: Vec<String>,
    /// Each cascade layer's rank: a rule's `layer` indexes it, and higher ranks win.
    pub layer_ranks: Vec<usize>,
    /// `@property` registrations, which give custom properties initial values.
    pub properties: Rc<Registrations>,
}

impl Parsed {
    /** What the cascade reads of the parser's state after every sheet. */
    fn of(state: &State) -> Self {
        Self {
            buckets: Buckets::new(&state.rules),
            pseudo_rules: state.rules.iter().any(|rule| rule.pseudo.is_some()),
            container_rules: state.rules.iter().any(|rule| !rule.containers.is_empty()),
            untrusted: state.budget.untrusted,
            rules: state.rules.clone(),
            keyframes: state.keyframes.clone(),
            font_faces: state.font_faces.faces.clone(),
            diagnostics: state.diagnostics.clone(),
            layer_ranks: state.layers.ranks(),
            properties: Rc::new(state.properties.clone()),
        }
    }
}

/** The parser after some sheets; the next sheet continues from here as if parsed in sequence. */
#[derive(Clone)]
struct State {
    /// Sheets parsed so far.
    count: usize,
    /// Bytes of the sheet allowance left; a sheet past it contributes only masks.
    bytes: usize,
    rules: Vec<Rc<Rule>>,
    budget: Budget,
    keyframes: Keyframes,
    font_faces: FontFaces,
    diagnostics: Vec<String>,
    /// Cascade layers in declaration order, shared by every sheet.
    layers: Layers,
    /// `@property` registrations, shared by every sheet.
    properties: Registrations,
}

impl Default for State {
    /** Before any sheet: the whole allowance. */
    fn default() -> Self {
        Self {
            count: 0,
            bytes: STYLE_BYTES * STYLE_SHEETS,
            rules: Vec::new(),
            budget: Budget::default(),
            keyframes: Keyframes::default(),
            font_faces: FontFaces::default(),
            diagnostics: Vec::new(),
            layers: Layers::default(),
            properties: Registrations::default(),
        }
    }
}

/** One recent parse: its context, its exact sheet texts, the parser after them and, once a
cascade asked for it, what the cascade reads of that parser. */
struct Recent {
    context: RenderContext,
    sheets: Vec<String>,
    state: Rc<State>,
    parsed: std::cell::OnceCell<Rc<Parsed>>,
    /// The last scoped list parsed on top of this parse, for the next cascade to reuse or extend.
    scoped: RefCell<Option<Rc<ScopedParse>>>,
}

/** One scoped sheet list parsed on top of a recent main parse, kept whole: the parser after it,
each rule's group and what the cascade reads. */
struct ScopedParse {
    scoped: Vec<(String, usize)>,
    group: usize,
    state: Rc<State>,
    groups: Vec<usize>,
    parsed: Rc<Parsed>,
}

thread_local! {
    /** Most recently used first. */
    static RECENT: RefCell<VecDeque<Recent>> = const { RefCell::new(VecDeque::new()) };
}

/**
 * The parse of `sheets` under `context`, followed by `scoped` shadow sheets, and each rule's
 * group (`scoped` rules take their sheet's group, the others `document_group`). The parse of
 * `sheets` comes from this thread's recent parses, exactly or by continuing the longest one
 * these sheets extend, and is kept for the next cascade; scoped sheets parse on top of it,
 * continuing the parse of the last scoped list they extend (a shadow-DOM page hands the cascade
 * the same hundreds of scoped sheets every step, plus the newly defined components').
 */
pub(super) fn parsed(
    sheets: &[String],
    scoped: &[(String, usize)],
    document_group: usize,
    context: RenderContext,
) -> EngineResult<(Rc<Parsed>, Vec<usize>)> {
    let main = match recent(sheets, context) {
        Some((state, true)) => state,
        found => {
            let state = found.map_or_else(State::default, |(state, _)| (*state).clone());
            let (state, _) = parse(state, sheets, &[], document_group, context)?;
            remember(sheets, context, state)
        }
    };
    if scoped.is_empty() {
        let parsed = RECENT.with_borrow(|recent| {
            recent
                .iter()
                .find(|entry| Rc::ptr_eq(&entry.state, &main))
                .map(|entry| Rc::clone(entry.parsed.get_or_init(|| Rc::new(Parsed::of(&main)))))
        });
        return Ok((
            parsed.unwrap_or_else(|| Rc::new(Parsed::of(&main))),
            Vec::new(),
        ));
    }
    let prior = RECENT.with_borrow(|recent| {
        recent
            .iter()
            .find(|entry| Rc::ptr_eq(&entry.state, &main))
            .and_then(|entry| entry.scoped.borrow().clone())
            .filter(|prior| prior.group == document_group)
    });
    // ponytail: the list compares are O(scoped bytes) per cascade; a digest from the
    // coordinator would make them O(1).
    if let Some(prior) = &prior
        && prior.scoped == scoped
    {
        return Ok((Rc::clone(&prior.parsed), prior.groups.clone()));
    }
    let (state, from, groups) = match &prior {
        Some(prior) if scoped.starts_with(&prior.scoped) => (
            (*prior.state).clone(),
            prior.scoped.len(),
            prior.groups.clone(),
        ),
        _ => ((*main).clone(), 0, Vec::new()),
    };
    let (state, groups) = parse_scoped(state, &scoped[from..], groups, document_group, context)?;
    let parsed = Rc::new(Parsed::of(&state));
    RECENT.with_borrow(|recent| {
        if let Some(entry) = recent.iter().find(|entry| Rc::ptr_eq(&entry.state, &main)) {
            entry.scoped.replace(Some(Rc::new(ScopedParse {
                scoped: scoped.to_vec(),
                group: document_group,
                state: Rc::new(state),
                groups: groups.clone(),
                parsed: Rc::clone(&parsed),
            })));
        }
    });
    Ok((parsed, groups))
}

/** Continues `state` over scoped sheets, extending `groups` (one per rule so far) with each
sheet's group; the rules before them take `document_group`. */
fn parse_scoped(
    mut state: State,
    scoped: &[(String, usize)],
    mut groups: Vec<usize>,
    document_group: usize,
    context: RenderContext,
) -> EngineResult<(State, Vec<usize>)> {
    groups.resize(state.rules.len(), document_group);
    for (sheet, group) in scoped {
        let shared = (
            &mut state.rules,
            &mut state.budget,
            &mut state.keyframes,
            &mut state.font_faces,
            &mut state.layers,
            &mut state.properties,
        );
        parse_sheet(sheet, context, shared, &mut state.diagnostics, false)?;
        groups.resize(state.rules.len(), *group);
    }
    Ok((state, groups))
}

/** The parser after the recent parse of exactly these sheets (true), or after the longest
recent list they extend (false), now the most recently used. */
fn recent(sheets: &[String], context: RenderContext) -> Option<(Rc<State>, bool)> {
    RECENT.with_borrow_mut(|recent| {
        let best = recent
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.context == context && sheets.starts_with(&entry.sheets))
            .max_by_key(|(_, entry)| entry.sheets.len())
            .map(|(index, _)| index)?;
        let entry = recent.remove(best)?;
        let found = (Rc::clone(&entry.state), entry.sheets.len() == sheets.len());
        recent.push_front(entry);
        Some(found)
    })
}

/** Keeps the parser after `sheets` as the most recent parse. */
fn remember(sheets: &[String], context: RenderContext, state: State) -> Rc<State> {
    let state = Rc::new(state);
    RECENT.with_borrow_mut(|recent| {
        recent.push_front(Recent {
            context,
            sheets: sheets.to_vec(),
            state: Rc::clone(&state),
            parsed: std::cell::OnceCell::new(),
            scoped: RefCell::new(None),
        });
        recent.truncate(PARSED_SHEETS);
    });
    state
}

/** Continues `state` over the sheets it has not parsed, then the scoped ones, under the shared
rule budget; returns the parser and each rule's group when there were scoped sheets. */
fn parse(
    mut state: State,
    sheets: &[String],
    scoped: &[(String, usize)],
    document_group: usize,
    context: RenderContext,
) -> EngineResult<(State, Vec<usize>)> {
    for (index, sheet) in sheets.iter().enumerate().skip(state.count) {
        // Sheets past the count or byte budget still contribute their hiding rules as masks.
        let skipped = index >= STYLE_SHEETS * 2 || sheet.len() > state.bytes;
        if !skipped {
            state.bytes -= sheet.len();
        }
        let shared = (
            &mut state.rules,
            &mut state.budget,
            &mut state.keyframes,
            &mut state.font_faces,
            &mut state.layers,
            &mut state.properties,
        );
        parse_sheet(sheet, context, shared, &mut state.diagnostics, skipped)?;
        state.count = index + 1;
    }
    if scoped.is_empty() {
        return Ok((state, Vec::new()));
    }
    parse_scoped(state, scoped, Vec::new(), document_group, context)
}

#[cfg(test)]
mod tests {
    use super::*;

    /** The same scoped list on the same main parse is one parse; a list that extends it keeps
    the earlier scoped rules' identity and groups and parses only the added sheets. */
    #[test]
    fn scoped_lists_are_reused_and_extended() {
        let sheets = vec!["p { color: red }".to_owned()];
        let context = RenderContext::default();
        let scoped = vec![("a { color: blue }".to_owned(), 7)];
        let (first, groups) = parsed(&sheets, &scoped, 0, context).unwrap();
        assert_eq!(groups, [0, 7]);
        let (again, _) = parsed(&sheets, &scoped, 0, context).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        let mut longer = scoped.clone();
        longer.push(("b { color: green }".to_owned(), 9));
        let (extended, groups) = parsed(&sheets, &longer, 0, context).unwrap();
        assert_eq!(groups, [0, 7, 9]);
        assert!(Rc::ptr_eq(&first.rules[1], &extended.rules[1]));
        assert_eq!(extended.rules.len(), 3);
        let (other_group, groups) = parsed(&sheets, &longer, 1, context).unwrap();
        assert!(!Rc::ptr_eq(&extended, &other_group));
        assert_eq!(groups, [1, 7, 9]);
    }

    /** The same texts under the same context share one parse; a changed text or context does not. */
    #[test]
    fn recent_parses_are_shared_by_exact_texts_and_context() {
        let sheets = vec!["p { color: red }".to_owned()];
        let context = RenderContext::default();
        let (first, _) = parsed(&sheets, &[], 0, context).unwrap();
        let (again, _) = parsed(&sheets, &[], 0, context).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        let (other, _) = parsed(&["p { color: blue }".to_owned()], &[], 0, context).unwrap();
        assert!(!Rc::ptr_eq(&first, &other));
        let narrow = RenderContext {
            width: 400.0,
            ..context
        };
        let (narrowed, _) = parsed(&sheets, &[], 0, narrow).unwrap();
        assert!(!Rc::ptr_eq(&first, &narrowed));
        assert_eq!(first.rules.len(), 1);
    }

    /** A list that extends a recent one continues its parse: the earlier rules keep their order
    and identity, and the new sheet's rules follow them. */
    #[test]
    fn extended_lists_continue_the_recent_parse() {
        let base = vec![
            "p { color: red }".to_owned(),
            "a { color: blue }".to_owned(),
        ];
        let context = RenderContext {
            height: 500.0,
            ..RenderContext::default()
        };
        let (first, _) = parsed(&base, &[], 0, context).unwrap();
        let mut longer = base.clone();
        longer.push("b { color: green }".to_owned());
        let (extended, _) = parsed(&longer, &[], 0, context).unwrap();
        assert_eq!(extended.rules.len(), 3);
        assert!(Rc::ptr_eq(&first.rules[1], &extended.rules[1]));
        assert_eq!(extended.rules[2].order, 2);
        let (fresh, _) = {
            let state = State::default();
            let (state, _) = parse(state, &longer, &[], 0, context).unwrap();
            (Rc::new(Parsed::of(&state)), ())
        };
        assert_eq!(fresh.rules.len(), extended.rules.len());
        assert_eq!(fresh.rules[2].specificity, extended.rules[2].specificity);
    }
}
