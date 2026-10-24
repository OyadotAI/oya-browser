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
    sheet::{Budget, Rule, parse_sheet},
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
}

thread_local! {
    /** Most recently used first. */
    static RECENT: RefCell<VecDeque<Recent>> = const { RefCell::new(VecDeque::new()) };
}

/**
 * The parse of `sheets` under `context`, followed by `scoped` shadow sheets, and each rule's
 * group (`scoped` rules take their sheet's group, the others `document_group`). The parse of
 * `sheets` comes from this thread's recent parses, exactly or by continuing the longest one
 * these sheets extend, and is kept for the next cascade; scoped sheets parse on top of it.
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
    let (state, groups) = parse((*main).clone(), sheets, scoped, document_group, context)?;
    Ok((Rc::new(Parsed::of(&state)), groups))
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
        );
        parse_sheet(sheet, context, shared, &mut state.diagnostics, skipped)?;
        state.count = index + 1;
    }
    let mut groups = Vec::new();
    if !scoped.is_empty() {
        groups.resize(state.rules.len(), document_group);
        for (sheet, group) in scoped {
            let shared = (
                &mut state.rules,
                &mut state.budget,
                &mut state.keyframes,
                &mut state.font_faces,
            );
            parse_sheet(sheet, context, shared, &mut state.diagnostics, false)?;
            groups.resize(state.rules.len(), *group);
        }
    }
    Ok((state, groups))
}

#[cfg(test)]
mod tests {
    use super::*;

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
