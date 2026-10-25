use super::container::Query;
/**
 * Stylesheet structure per CSS Syntax: style rules, CSS nesting and at-rules. Unsupported
 * selectors and at-rules are dropped with a diagnostic; when a dropped rule could have hidden
 * content, a mask rule keeps whatever it might target out of agent reach.
 */
use super::{
    animation::Keyframes,
    constants::BLOCK_DEPTH,
    declaration::{self, Block, Declaration},
    font_face::FontFaces,
    layer::{self, Layers},
    media::{self, Truth},
    note,
    selector_parse::{self, Compound, Selector},
    syntax::{block_end, find_top, split_top, strip_comments},
    vars::{Registration, Registrations},
};
use crate::{
    constants::{CSS_RULES, STYLE_BYTES},
    document::RenderContext,
    protocol::EngineResult,
};
use std::rc::Rc;
/** What a rule matches: a parsed selector, or the lenient subject of a dropped one. */
#[derive(Clone, Debug)]
pub(super) enum Target {
    /// A fully supported selector.
    Selector(Selector),
    /// A superset of a dropped selector's subjects; only used for masks.
    Mask(Compound),
}
/** One style rule per selector in a list, ordered for the cascade. */
#[derive(Clone, Debug)]
pub(super) struct Rule {
    /// What the rule matches.
    pub target: Target,
    /// Declarations applied to matches; empty for mask rules.
    pub declarations: Vec<Declaration>,
    /// Scalar specificity.
    pub specificity: usize,
    /// Source order across every sheet.
    pub order: usize,
    /// Matches must not be agent-actionable.
    pub hides: bool,
    /// Some declaration could hide its match, so an undecided match must mask it.
    pub hiding: bool,
    /// Styles the subject's `::before` (Some(false)) or `::after` (Some(true)) box instead of it.
    pub pseudo: Option<bool>,
    /// Ancestors a mask's dropped selector demands of its subject: one rules out the root
    /// wrapper, two the body, so an unsupported selector deep in the page never masks it whole.
    pub ancestors: usize,
    /// Enclosing `@container` queries, all of which must hold for the rule to apply.
    pub containers: Vec<Rc<Query>>,
    /// The cascade layer the rule sits in (an index into the sheets' `Layers`).
    pub layer: usize,
}
/** How a block's rules participate. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Rules apply normally.
    Apply,
    /// Under an undecidable condition: only hiding rules contribute, as masks.
    Masked,
}
/// Mask-only rules kept after the rule bound or from skipped sheets, so hiding rules never
/// fall away silently; past this the whole document stops being agent-actionable.
pub(super) const MASK_RULES: usize = CSS_RULES;
/// Bytes of skipped sheets scanned for hiding rules; a sheet past this fails closed.
pub(super) const MASK_SCAN_BYTES: usize =
    crate::constants::STYLE_BYTES * crate::constants::STYLE_SHEETS;
/** Rule bounds shared by every sheet of one cascade, and the fail-closed state they lead to. */
#[derive(Clone, Debug, Default)]
pub(super) struct Budget {
    /// Ordinary rules kept, bounded by CSS_RULES.
    pub ordinary: usize,
    /// Mask-only rules kept from the reserve, bounded by MASK_RULES.
    pub masks: usize,
    /// Bytes of skipped sheets scanned for hiding rules.
    pub scanned: usize,
    /// A hiding rule could not be kept: no node may be agent-actionable.
    pub untrusted: bool,
}
/** One sheet's parse state. */
struct Parser<'a> {
    /// Output rules, shared so a page's next cascade can continue from them.
    rules: &'a mut Vec<Rc<Rule>>,
    /// `@keyframes` rules by name, shared by every sheet.
    keyframes: &'a mut Keyframes,
    /// `@font-face` rules in source order, shared by every sheet.
    font_faces: &'a mut FontFaces,
    /// Shared bounds across sheets.
    budget: &'a mut Budget,
    /// Only hiding rules are kept, as masks: the sheet itself was over a bound.
    masks_only: bool,
    /// Value-free diagnostics.
    diagnostics: &'a mut Vec<String>,
    /// Viewport and appearance for @media.
    context: RenderContext,
    /// `@container` queries enclosing the current block, outermost first.
    containers: Vec<Rc<Query>>,
    /// Cascade layers declared so far, shared by every sheet.
    layers: &'a mut Layers,
    /// The layer enclosing the current block.
    layer: usize,
    /// `@property` registrations, shared by every sheet.
    properties: &'a mut Registrations,
    /// The sheet declared `@namespace`, which Eye does not resolve: prefixed selectors drop.
    namespaced: bool,
}
/** Bounds the sheet, then parses every rule it can. */
pub(super) fn parse_sheet(
    sheet: &str,
    context: RenderContext,
    (rules, budget, keyframes, font_faces, layers, properties): (
        &mut Vec<Rc<Rule>>,
        &mut Budget,
        &mut Keyframes,
        &mut FontFaces,
        &mut Layers,
        &mut Registrations,
    ),
    diagnostics: &mut Vec<String>,
    skipped: bool,
) -> EngineResult<()> {
    let masks_only = skipped || sheet.len() > STYLE_BYTES;
    if masks_only {
        note(
            diagnostics,
            "A stylesheet past the style budget was skipped; only its hiding rules apply.".into(),
        );
        if budget.scanned.saturating_add(sheet.len()) > MASK_SCAN_BYTES {
            fail_closed(budget, diagnostics);
            return Ok(());
        }
        budget.scanned += sheet.len();
    }
    let source = strip_comments(sheet);
    let mut parser = Parser {
        rules,
        keyframes,
        font_faces,
        budget,
        masks_only,
        diagnostics,
        context,
        containers: Vec::new(),
        layers,
        layer: layer::OUTER,
        properties,
        namespaced: false,
    };
    parser.list(&source, None, Mode::Apply, 0)
}
impl Parser<'_> {
    /** A rule list; with `parents`, bare declarations and nested rules belong to them. */
    fn list(
        &mut self,
        source: &str,
        parents: Option<&[String]>,
        mode: Mode,
        depth: usize,
    ) -> EngineResult<()> {
        if depth > BLOCK_DEPTH {
            note(self.diagnostics, "Deeply nested CSS was ignored.".into());
            return Ok(());
        }
        let mut index = 0;
        let mut block = Block::default();
        while index < source.len() {
            let start = index;
            let stop = find_top(source, start, b"{;}");
            let prelude = source[start..stop].trim();
            if stop >= source.len() || source.as_bytes()[stop] != b'{' {
                index = stop + 1;
                if parents.is_some() && !prelude.starts_with('@') {
                    declaration::declaration(prelude, &mut block, self.diagnostics);
                } else if prelude.starts_with('@') {
                    self.statement(prelude);
                }
                continue;
            }
            let end = block_end(source, stop);
            let body = source.get(stop + 1..end).unwrap_or("");
            index = end + 1;
            if parents.is_some() && prelude.starts_with("--") {
                declaration::declaration(
                    &source[start..end.min(source.len())],
                    &mut block,
                    self.diagnostics,
                );
            } else {
                // Declarations before a nested rule apply before it; any after it form a
                // nested declarations rule (Blink's CSSNestedDeclarationsRule) in source order.
                if let Some(parents) = parents {
                    self.push(parents, &std::mem::take(&mut block), mode)?;
                }
                if prelude.starts_with('@') {
                    self.at_rule(prelude, body, parents, mode, depth)?;
                } else {
                    self.style_rule(prelude, body, parents, mode, depth)?;
                }
            }
        }
        if let Some(parents) = parents {
            self.push(parents, &block, mode)?;
        }
        Ok(())
    }
    /** Statement at-rules: `@import` and `@namespace` are dropped; `@charset` and `@layer` lists are inert. */
    fn statement(&mut self, prelude: &str) {
        let name = at_name(prelude);
        if name == "layer" {
            // `@layer a, b;` fixes the order of layers before any of their rules appear.
            for layer in split_top(prelude["@layer".len()..].trim(), b',') {
                self.layers.declare(self.layer, layer.trim());
            }
        } else if name != "charset" {
            self.namespaced |= name == "namespace";
            note(self.diagnostics, unsupported_at(&name));
        }
    }
    /** Conditional at-rules are evaluated; others are dropped, masked when they could hold hiding rules. */
    fn at_rule(
        &mut self,
        prelude: &str,
        body: &str,
        parents: Option<&[String]>,
        mode: Mode,
        depth: usize,
    ) -> EngineResult<()> {
        let name = at_name(prelude);
        let condition = prelude[1 + name.len().min(prelude.len() - 1)..].trim();
        let truth = match name.as_str() {
            "media" => media::media(condition, self.context),
            "supports" => match media::supports(condition) {
                Truth::False => Truth::Unknown,
                truth => truth,
            },
            "layer" => {
                let enclosing = self.layer;
                self.layer = match condition {
                    "" => self.layers.anonymous(enclosing),
                    named => self.layers.declare(enclosing, named),
                };
                let result = self.list(body, parents, mode, depth + 1);
                self.layer = enclosing;
                return result;
            }
            "container" => match Query::parse(condition) {
                Some(query) => {
                    self.containers.push(Rc::new(query));
                    let result = self.list(body, parents, mode, depth + 1);
                    self.containers.pop();
                    return result;
                }
                None => {
                    note(self.diagnostics, unsupported_at(&name));
                    Truth::Unknown
                }
            },
            "keyframes" | "-webkit-keyframes" => {
                // Keyframes are global, so a container condition that differs per element masks them.
                let masked = mode == Mode::Masked || self.masks_only || !self.containers.is_empty();
                self.keyframes
                    .parse(prelude, body, masked, self.diagnostics);
                return Ok(());
            }
            "font-face" => {
                // A face under an unevaluated condition or in a skipped sheet stays unknown:
                // its family then falls back, which is what a dropped rule means for text.
                if mode == Mode::Apply && !self.masks_only {
                    self.font_faces.parse(body, self.diagnostics);
                }
                return Ok(());
            }
            "property" => {
                // Registrations are global, like keyframes; under a condition they stay unknown.
                if mode == Mode::Apply && !self.masks_only && self.containers.is_empty() {
                    register(self.properties, condition, body);
                }
                return Ok(());
            }
            "page"
            | "counter-style"
            | "font-feature-values"
            | "font-palette-values"
            | "view-transition"
            | "starting-style"
            | "position-try" => {
                note(self.diagnostics, unsupported_at(&name));
                return Ok(());
            }
            _ => {
                note(self.diagnostics, unsupported_at(&name));
                Truth::Unknown
            }
        };
        let mode = match truth {
            Truth::True => mode,
            Truth::Unknown => Mode::Masked,
            Truth::False => return Ok(()),
        };
        self.list(body, parents, mode, depth + 1)
    }
    /** A style rule: resolve nesting, then its own declarations and nested rules. */
    fn style_rule(
        &mut self,
        prelude: &str,
        body: &str,
        parents: Option<&[String]>,
        mode: Mode,
        depth: usize,
    ) -> EngineResult<()> {
        let (selectors, complete) = nest(prelude, parents);
        if !complete {
            // Selectors past the bound could be hiding ones: fail closed rather than drop them.
            fail_closed(self.budget, self.diagnostics);
        }
        if selectors.is_empty() {
            return Ok(());
        }
        self.list(body, Some(&selectors), mode, depth + 1)
    }
    /** Pushes one rule per selector, or masks for dropped and masked hiding rules. */
    fn push(&mut self, selectors: &[String], block: &Block, mode: Mode) -> EngineResult<()> {
        if block.declarations.is_empty() && !block.hiding {
            return Ok(());
        }
        for text in selectors {
            let (text, pseudo) = generated(text);
            // Under `@namespace` a prefix (or the default namespace) changes what `*|E` means.
            let parsed = match self.namespaced && prefixed(&text) {
                true => Err("namespace selector"),
                false => selector_parse::parse(&text),
            };
            let rules: Vec<Rule> = match (parsed, mode) {
                (Ok(selector), Mode::Apply) => vec![Rule {
                    specificity: selector_parse::specificity(&selector),
                    target: Target::Selector(selector),
                    declarations: block.declarations.clone(),
                    order: 0,
                    hides: block.hides && pseudo.is_none(),
                    hiding: block.hiding && pseudo.is_none(),
                    pseudo,
                    ancestors: 0,
                    containers: self.containers.clone(),
                    layer: self.layer,
                }],
                (Ok(_), Mode::Masked) if pseudo.is_some() => Vec::new(),
                (Ok(selector), Mode::Masked) => {
                    { block.hiding.then(|| mask(Target::Selector(selector))) }
                        .into_iter()
                        .collect()
                }
                (Err(feature), _) => {
                    note(
                        self.diagnostics,
                        format!(
                            "CSS selector feature {feature} is not supported; its rule was ignored."
                        ),
                    );
                    // Hiding a `::before`/`::after` box never hides its element, so no mask.
                    let subjects = match block.hiding && pseudo.is_none() {
                        true => super::subjects::subjects(&text),
                        false => Vec::new(),
                    };
                    subjects
                        .into_iter()
                        .map(|(compound, ancestors)| Rule {
                            ancestors,
                            ..mask(Target::Mask(compound))
                        })
                        .collect()
                }
            };
            for mut rule in rules {
                // Past the rule bound only hiding rules stay, as masks, so nothing they hid
                // becomes actionable; past the mask reserve the document fails closed.
                let ordinary = !rule.declarations.is_empty() || rule.pseudo.is_some();
                if ordinary && (self.masks_only || self.budget.ordinary >= CSS_RULES) {
                    if !self.masks_only {
                        note(
                            self.diagnostics,
                            format!("CSS rules after the first {CSS_RULES} were ignored."),
                        );
                    }
                    if !block.hiding || rule.pseudo.is_some() {
                        continue;
                    }
                    rule = mask(rule.target);
                }
                if rule.declarations.is_empty() && rule.pseudo.is_none() {
                    if self.budget.masks >= MASK_RULES {
                        fail_closed(self.budget, self.diagnostics);
                        continue;
                    }
                    self.budget.masks += 1;
                } else {
                    self.budget.ordinary += 1;
                }
                rule.order = self.rules.len();
                self.rules.push(Rc::new(rule));
            }
        }
        Ok(())
    }
}
/** Splits a trailing `::before` or `::after` off a selector: the subject's selector and which box it styles. */
fn generated(text: &str) -> (String, Option<bool>) {
    let trimmed = text.trim_end();
    let lower = trimmed.to_ascii_lowercase();
    for (suffix, after) in [
        ("::before", false),
        (":before", false),
        ("::after", true),
        (":after", true),
    ] {
        if lower.ends_with(suffix) {
            let rest = trimmed[..trimmed.len() - suffix.len()].trim_end();
            let subject = if rest.is_empty() || rest.ends_with([' ', '>', '+', '~']) {
                format!("{rest}*")
            } else {
                rest.to_string()
            };
            return (subject, Some(after));
        }
    }
    (text.to_string(), None)
}
/**
 * `@property --name { syntax; inherits; initial-value }`, validated as Blink's
 * PropertyRegistration does for what Eye uses: `syntax` and `inherits` are required, and
 * `initial-value` too unless the syntax is the universal `"*"`. The first valid rule wins.
 */
fn register(properties: &mut Registrations, name: &str, body: &str) {
    if !name.starts_with("--") || properties.contains_key(name) {
        return;
    }
    let mut fields = std::collections::BTreeMap::new();
    for declaration in split_top(body, b';') {
        if let Some((key, value)) = declaration.split_once(':') {
            fields.insert(key.trim().to_ascii_lowercase(), value.trim());
        }
    }
    let universal = fields
        .get("syntax")
        .is_some_and(|syntax| syntax.trim_matches(['"', '\'']).trim() == "*");
    let inherits = match fields.get("inherits").map(|v| v.to_ascii_lowercase()) {
        Some(value) if value == "true" => true,
        Some(value) if value == "false" => false,
        _ => return,
    };
    let initial = fields.get("initial-value").map(|value| Rc::from(*value));
    if !fields.contains_key("syntax") || (initial.is_none() && !universal) {
        return;
    }
    properties.insert(name.to_owned(), Registration { inherits, initial });
}
/** Marks the cascade untrusted: some hiding rule could not be kept. */
fn fail_closed(budget: &mut Budget, diagnostics: &mut Vec<String>) {
    if !budget.untrusted {
        note(
            diagnostics,
            "Too much CSS to check what it hides; agents cannot act on this page.".into(),
        );
    }
    budget.untrusted = true;
}
/** A rule that applies nothing and only withdraws agent actionability. */
fn mask(target: Target) -> Rule {
    Rule {
        target,
        declarations: Vec::new(),
        specificity: 0,
        order: 0,
        hides: true,
        hiding: true,
        pseudo: None,
        ancestors: 0,
        containers: Vec::new(),
        layer: layer::OUTER,
    }
}
/** Lowercase at-rule name after `@`. */
fn at_name(prelude: &str) -> String {
    prelude
        .get(1..)
        .unwrap_or("")
        .split(|c: char| c.is_whitespace() || c == '(' || c == '{' || c == ';')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}
/** Value-free at-rule diagnostic. */
fn unsupported_at(name: &str) -> String {
    format!(
        "CSS at-rule @{} is not supported and was ignored.",
        super::diagnostic_name(name)
    )
}
/**
 * CSS nesting as Blink resolves it (CSSSelectorParser with a parent rule): a nested selector
 * that is relative (`> a`) or has no `&` gets `& ` in front, and `&` stands for the whole parent
 * list as `:is(parents)`, with that list's highest specificity. A single parent at the very start
 * is spliced in as text, which matches and weighs the same and keeps rules bucketed by subject.
 * False when cut at the rule bound.
 */
fn nest(prelude: &str, parents: Option<&[String]>) -> (Vec<String>, bool) {
    let pieces = split_top(prelude, b',');
    let Some(parents) = parents else {
        return (pieces.into_iter().map(Into::into).collect(), true);
    };
    let mut out = Vec::new();
    for piece in pieces {
        if out.len() >= CSS_RULES {
            return (out, false);
        }
        let piece = piece.trim();
        let mut ampersands = nesting_selectors(piece);
        let piece = if ampersands.is_empty() {
            ampersands.push(0);
            format!("& {piece}")
        } else {
            piece.to_owned()
        };
        out.push(match (parents, ampersands.as_slice()) {
            ([parent], [0]) => format!("{parent}{}", &piece[1..]),
            _ => splice(&piece, &ampersands, &format!(":is({})", parents.join(", "))),
        });
    }
    (out, true)
}
/** Byte offsets of each `&` outside strings and escapes. */
fn nesting_selectors(selector: &str) -> Vec<usize> {
    let bytes = selector.as_bytes();
    let (mut out, mut index, mut quote) = (Vec::new(), 0, None);
    while index < bytes.len() {
        match (bytes[index], quote) {
            (b'\\', _) => index += 1,
            (b'"' | b'\'', None) => quote = Some(bytes[index]),
            (byte, Some(open)) if byte == open => quote = None,
            (b'&', None) => out.push(index),
            _ => {}
        }
        index += 1;
    }
    out
}
/** `selector` with the `&` at each of `at` replaced by `parent`. */
fn splice(selector: &str, at: &[usize], parent: &str) -> String {
    let mut out = String::with_capacity(selector.len() + parent.len() * at.len());
    let mut from = 0;
    for &index in at {
        out.push_str(&selector[from..index]);
        out.push_str(parent);
        from = index + 1;
    }
    out.push_str(&selector[from..]);
    out
}
/** Whether a selector carries a namespace prefix (`|` other than the `|=` operator). */
fn prefixed(selector: &str) -> bool {
    selector
        .match_indices('|')
        .any(|(at, _)| selector.as_bytes().get(at + 1) != Some(&b'='))
}
