/**
 * CSS selectors for querySelector, matches and closest: compound selectors (type, universal, id,
 * class, attribute operators, structural and state pseudo-classes, :not/:is/:where/:has) joined by
 * descendant, child, adjacent and general-sibling combinators. Anything else is a SyntaxError,
 * as browsers throw, so libraries fall back to their own engines.
 */
use super::Page;

/** A comma-separated selector list. */
#[derive(Clone, Debug)]
pub(in crate::v9) struct SelectorList(Vec<Complex>);

/** Compounds joined by combinators, stored left to right. */
#[derive(Clone, Debug)]
struct Complex(Vec<(Compound, Combinator)>);

/** How a compound relates to the one before it. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Combinator {
    None,
    Descendant,
    Child,
    Adjacent,
    Sibling,
}

/** One compound selector. */
#[derive(Clone, Debug, Default)]
struct Compound {
    tag: Option<String>,
    ids: Vec<String>,
    classes: Vec<String>,
    attributes: Vec<Attribute>,
    pseudos: Vec<Pseudo>,
}

/** `[name op value i]` */
#[derive(Clone, Debug)]
struct Attribute {
    name: String,
    op: Option<char>,
    value: String,
    insensitive: bool,
}

/** Supported pseudo-classes. */
#[derive(Clone, Debug)]
enum Pseudo {
    Nth {
        a: i64,
        b: i64,
        of_type: bool,
        last: bool,
    },
    Only {
        of_type: bool,
    },
    Not(SelectorList),
    Is(SelectorList),
    Has(SelectorList),
    Empty,
    Root,
    Scope,
    Checked,
    Disabled,
    Enabled,
    Never,
}

/** Parse a selector list; the error text becomes a SyntaxError message. */
pub(in crate::v9) fn parse(text: &str) -> Result<SelectorList, String> {
    let mut parser = Parser {
        chars: text.chars().collect(),
        position: 0,
        depth: 0,
    };
    let list = parser.list()?;
    parser.skip();
    if parser.position < parser.chars.len() {
        return Err(format!("'{text}' is not a valid selector"));
    }
    Ok(list)
}

/** Selector parser state. */
struct Parser {
    chars: Vec<char>,
    position: usize,
    depth: usize,
}

/** Nesting of :not/:is/:has the parser follows. */
const SELECTOR_DEPTH: usize = 32;

impl Parser {
    /** The current character. */
    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }
    /** Skip whitespace; returns whether any was skipped. */
    fn skip(&mut self) -> bool {
        let start = self.position;
        while self.peek().is_some_and(char::is_whitespace) {
            self.position += 1;
        }
        self.position > start
    }
    /** A list up to the end or a closing parenthesis. */
    fn list(&mut self) -> Result<SelectorList, String> {
        self.depth += 1;
        if self.depth > SELECTOR_DEPTH {
            return Err("selector nested too deeply".into());
        }
        let mut list = vec![self.complex()?];
        loop {
            self.skip();
            if self.peek() == Some(',') {
                self.position += 1;
                list.push(self.complex()?);
            } else {
                break;
            }
        }
        self.depth -= 1;
        Ok(SelectorList(list))
    }
    /** One complex selector. */
    fn complex(&mut self) -> Result<Complex, String> {
        self.skip();
        let mut parts = Vec::new();
        let mut combinator = Combinator::None;
        loop {
            let compound = self.compound()?;
            parts.push((compound, combinator));
            let spaced = self.skip();
            combinator = match self.peek() {
                Some('>') => Combinator::Child,
                Some('+') => Combinator::Adjacent,
                Some('~') => Combinator::Sibling,
                Some(',' | ')') | None => break,
                _ if spaced => Combinator::Descendant,
                _ => return Err("unexpected character in selector".into()),
            };
            if combinator != Combinator::Descendant {
                self.position += 1;
                self.skip();
            }
        }
        Ok(Complex(parts))
    }
    /** An identifier with CSS escapes. */
    fn ident(&mut self) -> Result<String, String> {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == '\\' {
                self.position += 1;
                let mut hex = String::new();
                while hex.len() < 6 && self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                    hex.push(self.peek().expect("hex digit"));
                    self.position += 1;
                }
                if hex.is_empty() {
                    if let Some(c) = self.peek() {
                        out.push(c);
                        self.position += 1;
                    }
                } else {
                    out.push(
                        u32::from_str_radix(&hex, 16)
                            .ok()
                            .and_then(char::from_u32)
                            .unwrap_or('\u{fffd}'),
                    );
                    if self.peek() == Some(' ') {
                        self.position += 1;
                    }
                }
            } else if c.is_alphanumeric() || c == '-' || c == '_' || !c.is_ascii() {
                out.push(c);
                self.position += 1;
            } else {
                break;
            }
        }
        if out.is_empty() {
            return Err("expected an identifier in selector".into());
        }
        Ok(out)
    }
    /** One compound. */
    fn compound(&mut self) -> Result<Compound, String> {
        let mut compound = Compound::default();
        let start = self.position;
        match self.peek() {
            Some('*') => {
                self.position += 1;
            }
            Some(c) if c.is_alphabetic() || c == '\\' || c == '_' || c == '-' => {
                compound.tag = Some(self.ident()?.to_ascii_lowercase());
            }
            _ => {}
        }
        loop {
            match self.peek() {
                Some('#') => {
                    self.position += 1;
                    compound.ids.push(self.ident()?);
                }
                Some('.') => {
                    self.position += 1;
                    compound.classes.push(self.ident()?);
                }
                Some('[') => {
                    self.position += 1;
                    compound.attributes.push(self.attribute()?);
                }
                Some(':') => {
                    self.position += 1;
                    if self.peek() == Some(':') {
                        return Err("pseudo-elements are not supported in selectors".into());
                    }
                    compound.pseudos.push(self.pseudo()?);
                }
                _ => break,
            }
        }
        if self.position == start {
            return Err("expected a selector".into());
        }
        Ok(compound)
    }
    /** An attribute selector after `[`. */
    fn attribute(&mut self) -> Result<Attribute, String> {
        self.skip();
        let name = self.ident()?.to_ascii_lowercase();
        self.skip();
        let mut op = None;
        let mut value = String::new();
        if self.peek() != Some(']') {
            let c = self.peek().ok_or("unterminated attribute selector")?;
            if c == '=' {
                op = Some('=');
                self.position += 1;
            } else if matches!(c, '~' | '|' | '^' | '$' | '*')
                && self.chars.get(self.position + 1) == Some(&'=')
            {
                op = Some(c);
                self.position += 2;
            } else {
                return Err("invalid attribute selector".into());
            }
            self.skip();
            value = match self.peek() {
                Some(quote @ ('"' | '\'')) => {
                    self.position += 1;
                    let mut text = String::new();
                    while let Some(c) = self.peek() {
                        self.position += 1;
                        if c == quote {
                            break;
                        }
                        if c == '\\' {
                            if let Some(next) = self.peek() {
                                text.push(next);
                                self.position += 1;
                            }
                            continue;
                        }
                        text.push(c);
                    }
                    text
                }
                _ => self.ident()?,
            };
            self.skip();
        }
        let mut insensitive = false;
        if matches!(self.peek(), Some('i' | 'I')) {
            insensitive = true;
            self.position += 1;
            self.skip();
        } else if matches!(self.peek(), Some('s' | 'S')) {
            self.position += 1;
            self.skip();
        }
        if self.peek() != Some(']') {
            return Err("unterminated attribute selector".into());
        }
        self.position += 1;
        Ok(Attribute {
            name,
            op,
            value,
            insensitive,
        })
    }
    /** A pseudo-class after `:`. */
    fn pseudo(&mut self) -> Result<Pseudo, String> {
        let name = self.ident()?.to_ascii_lowercase();
        let nth = |of_type, last| (of_type, last);
        let functional = self.peek() == Some('(');
        if functional {
            self.position += 1;
            self.skip();
        }
        let pseudo = match (name.as_str(), functional) {
            ("first-child", false) => Pseudo::Nth {
                a: 0,
                b: 1,
                of_type: false,
                last: false,
            },
            ("last-child", false) => Pseudo::Nth {
                a: 0,
                b: 1,
                of_type: false,
                last: true,
            },
            ("first-of-type", false) => Pseudo::Nth {
                a: 0,
                b: 1,
                of_type: true,
                last: false,
            },
            ("last-of-type", false) => Pseudo::Nth {
                a: 0,
                b: 1,
                of_type: true,
                last: true,
            },
            ("only-child", false) => Pseudo::Only { of_type: false },
            ("only-of-type", false) => Pseudo::Only { of_type: true },
            ("empty", false) => Pseudo::Empty,
            ("root", false) => Pseudo::Root,
            ("scope", false) => Pseudo::Scope,
            ("checked", false) => Pseudo::Checked,
            ("disabled", false) => Pseudo::Disabled,
            ("enabled", false) => Pseudo::Enabled,
            (
                "hover" | "focus" | "active" | "visited" | "focus-within" | "focus-visible"
                | "target",
                false,
            ) => Pseudo::Never,
            ("link" | "any-link", false) => {
                Pseudo::Is(parse("a[href],area[href]").expect("builtin selector"))
            }
            ("nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type", true) => {
                let (of_type, last) = nth(name.contains("of-type"), name.contains("last"));
                let (a, b) = self.an_plus_b()?;
                Pseudo::Nth {
                    a,
                    b,
                    of_type,
                    last,
                }
            }
            ("not", true) => Pseudo::Not(self.list()?),
            ("is" | "where" | "matches" | "any", true) => Pseudo::Is(self.list()?),
            ("has", true) => Pseudo::Has(self.list()?),
            _ => return Err(format!("unsupported pseudo-class :{name}")),
        };
        if functional {
            self.skip();
            if self.peek() != Some(')') {
                return Err("expected ')' in selector".into());
            }
            self.position += 1;
        }
        Ok(pseudo)
    }
    /** `an+b`, `odd`, `even`. */
    fn an_plus_b(&mut self) -> Result<(i64, i64), String> {
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if c == ')' {
                break;
            }
            if !c.is_whitespace() {
                text.push(c.to_ascii_lowercase());
            }
            self.position += 1;
        }
        let result = match text.as_str() {
            "odd" => (2, 1),
            "even" => (2, 0),
            _ => match text.split_once('n') {
                Some((a, b)) => {
                    let a = match a {
                        "" | "+" => 1,
                        "-" => -1,
                        a => a.parse().map_err(|_| "invalid nth expression")?,
                    };
                    let b = if b.is_empty() {
                        0
                    } else {
                        b.parse().map_err(|_| "invalid nth expression")?
                    };
                    (a, b)
                }
                None => (0, text.parse().map_err(|_| "invalid nth expression")?),
            },
        };
        Ok(result)
    }
}

/** Whether a node matches any selector of a list; `scope` is the :scope element. */
pub(in crate::v9) fn matches(
    page: &mut Page,
    node: usize,
    list: &SelectorList,
    scope: Option<usize>,
) -> bool {
    list.0
        .iter()
        .any(|complex| complex_matches(page, node, &complex.0, scope))
}

/** Right-to-left matching of a complex selector. */
fn complex_matches(
    page: &mut Page,
    node: usize,
    parts: &[(Compound, Combinator)],
    scope: Option<usize>,
) -> bool {
    let Some(((compound, combinator), rest)) = parts.split_last() else {
        return true;
    };
    if !compound_matches(page, node, compound, scope) {
        return false;
    }
    if rest.is_empty() {
        return true;
    }
    match combinator {
        Combinator::None => true,
        Combinator::Child => page
            .parent(node)
            .is_some_and(|parent| complex_matches(page, parent, rest, scope)),
        Combinator::Descendant => {
            let ancestors = page.ancestors(node);
            ancestors
                .into_iter()
                .any(|ancestor| complex_matches(page, ancestor, rest, scope))
        }
        Combinator::Adjacent => element_siblings(page, node, true)
            .last()
            .is_some_and(|sibling| complex_matches(page, *sibling, rest, scope)),
        Combinator::Sibling => {
            let before = element_siblings(page, node, true);
            before
                .into_iter()
                .any(|sibling| complex_matches(page, sibling, rest, scope))
        }
    }
}

/** Element siblings before (or after) a node, in document order. */
fn element_siblings(page: &mut Page, node: usize, before: bool) -> Vec<usize> {
    let Some(parent) = page.parent(node) else {
        return Vec::new();
    };
    let children = page.children(parent);
    let position = children
        .iter()
        .position(|child| *child == node)
        .unwrap_or(0);
    let slice = if before {
        &children[..position]
    } else {
        &children[position + 1..]
    };
    slice
        .iter()
        .copied()
        .filter(|child| page.is_element(*child))
        .collect()
}

/** Whether a node matches one compound. */
fn compound_matches(
    page: &mut Page,
    node: usize,
    compound: &Compound,
    scope: Option<usize>,
) -> bool {
    let Some(view) = page.view(node) else {
        return false;
    };
    if view.tag.starts_with('#') {
        return false;
    }
    if compound.tag.as_ref().is_some_and(|tag| *tag != view.tag) {
        return false;
    }
    let id = view.attributes.get("id").map(String::as_str).unwrap_or("");
    if compound.ids.iter().any(|wanted| wanted != id) {
        return false;
    }
    let classes = view
        .attributes
        .get("class")
        .map(String::as_str)
        .unwrap_or("");
    if compound.classes.iter().any(|wanted| {
        !classes
            .split_ascii_whitespace()
            .any(|class| class == wanted)
    }) {
        return false;
    }
    for attribute in &compound.attributes {
        let Some(actual) = view.attributes.get(&attribute.name) else {
            return false;
        };
        if !attribute_matches(attribute, actual) {
            return false;
        }
    }
    compound
        .pseudos
        .iter()
        .all(|pseudo| pseudo_matches(page, node, &view, pseudo, scope))
}

/** One attribute operator. */
fn attribute_matches(attribute: &Attribute, actual: &str) -> bool {
    let (actual, wanted) = if attribute.insensitive {
        (actual.to_lowercase(), attribute.value.to_lowercase())
    } else {
        (actual.to_string(), attribute.value.clone())
    };
    match attribute.op {
        None => true,
        Some('=') => actual == wanted,
        Some('~') => actual.split_ascii_whitespace().any(|word| word == wanted),
        Some('|') => actual == wanted || actual.starts_with(&format!("{wanted}-")),
        Some('^') => !wanted.is_empty() && actual.starts_with(&wanted),
        Some('$') => !wanted.is_empty() && actual.ends_with(&wanted),
        Some('*') => !wanted.is_empty() && actual.contains(&wanted),
        _ => false,
    }
}

/** One pseudo-class. */
fn pseudo_matches(
    page: &mut Page,
    node: usize,
    view: &super::nodes::NodeView,
    pseudo: &Pseudo,
    scope: Option<usize>,
) -> bool {
    match pseudo {
        Pseudo::Nth {
            a,
            b,
            of_type,
            last,
        } => {
            let mut siblings = element_siblings(page, node, !*last);
            if *of_type {
                siblings.retain(|sibling| page.tag(*sibling) == view.tag);
            }
            let position = siblings.len() as i64 + 1;
            if *a == 0 {
                return position == *b;
            }
            let n = (position - b) as f64 / *a as f64;
            n >= 0.0 && n.fract() == 0.0
        }
        Pseudo::Only { of_type } => {
            let mut before = element_siblings(page, node, true);
            let mut after = element_siblings(page, node, false);
            if *of_type {
                before.retain(|sibling| page.tag(*sibling) == view.tag);
                after.retain(|sibling| page.tag(*sibling) == view.tag);
            }
            before.is_empty() && after.is_empty()
        }
        Pseudo::Not(list) => !matches(page, node, list, scope),
        Pseudo::Is(list) => matches(page, node, list, scope),
        Pseudo::Has(list) => page
            .descendants(node)
            .into_iter()
            .any(|child| matches(page, child, list, scope)),
        Pseudo::Empty => page.children(node).iter().all(|child| {
            page.tag(*child) == "#comment"
                || page.tag(*child) == "#text" && page.text_content(*child).is_empty()
        }),
        Pseudo::Root => node == super::HTML,
        Pseudo::Scope => scope.map_or(node == super::HTML, |scope| scope == node),
        Pseudo::Checked => {
            view.attributes.contains_key("checked") || view.attributes.contains_key("selected")
        }
        Pseudo::Disabled => view.attributes.contains_key("disabled"),
        Pseudo::Enabled => {
            matches!(
                view.tag.as_str(),
                "input" | "button" | "select" | "textarea" | "option" | "fieldset"
            ) && !view.attributes.contains_key("disabled")
        }
        Pseudo::Never => false,
    }
}
