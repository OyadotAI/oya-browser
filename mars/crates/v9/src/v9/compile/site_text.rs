/** The source text V8 prints for an expression in a runtime error (`o.f is not a function`,
`x.y is not iterable`), rendered the way its CallPrinter renders the AST: identifiers and member
chains as written, literals as values, operators in parentheses, and anything it does not print
as `(intermediate value)`. Pages fingerprinting the engine compare these texts byte for byte, so
the rules follow V8's printer, quirks included. */
use crate::v9::syntax::{
    BinaryOp, Element, Expr, ExprKind, LogicalOp, MemberProp, Pattern, PropDef, UnaryOp,
};

/** What V8 prints for a sub-expression it does not render. */
pub(super) const INTERMEDIATE: &str = "(intermediate value)";
/** V8's name for the hidden binding that holds each value of a `for` loop head. */
pub(super) const FOR_VALUE: &str = ".for";
/** V8's name for the hidden binding that holds a caught exception. */
pub(super) const CATCH_VALUE: &str = ".catch";
/** What V8 prints for the operand of `yield*` when it is more than a name or a literal. */
pub(super) const YIELD_STAR_VALUE: &str = "yield* (intermediate value)";
/** Nesting depth past which an expression prints as an intermediate value, bounding the walk. */
const DEPTH: usize = 64;
/** The parts of a conditional, each an intermediate value to V8's printer. */
const CONDITIONAL_PARTS: usize = 3;

/** The expression as V8 prints it when it is the operand of a failing call, construction,
iteration or destructuring. */
pub(super) fn site_text(expr: &Expr) -> String {
    let mut out = String::new();
    print(expr, &mut out, DEPTH);
    out
}

/** Whether V8 prints this expression by its own text rather than as an intermediate value when
it is the value of an array destructuring; its printer finds only names and literals there. */
pub(super) fn is_name_or_literal(expr: &Expr) -> bool {
    matches!(
        expr.kind,
        ExprKind::Ident(_)
            | ExprKind::Number(_)
            | ExprKind::String(_)
            | ExprKind::Null
            | ExprKind::Bool(_)
            | ExprKind::This
    )
}

/** Whether V8's printer reaches the value of an array destructuring declaration: it looks for
the value at the position its iteration starts from, which an expression whose node sits at its
first token has, and one whose node sits at an operator (`x.y`, `a + b`, `a ? b : c`) does not. */
pub(super) fn starts_at_first_token(expr: &Expr) -> bool {
    !matches!(
        expr.kind,
        ExprKind::Member(..)
            | ExprKind::Chain(_)
            | ExprKind::Binary(..)
            | ExprKind::Logical(..)
            | ExprKind::Conditional(..)
            | ExprKind::Assign(..)
            | ExprKind::Sequence(_)
            | ExprKind::Update(_, false, _)
    )
}

/** The callee of a call expression, or none when the expression is not a call. */
pub(super) fn call_callee(expr: &Expr) -> Option<&Expr> {
    match &expr.kind {
        ExprKind::Call(callee, ..) => Some(callee),
        ExprKind::Chain(inner) => call_callee(inner),
        _ => None,
    }
}

/** Print the expression, or `(intermediate value)` when the printer has nothing for it. */
fn print(expr: &Expr, out: &mut String, depth: usize) {
    let before = out.len();
    if depth > 0 {
        visit(expr, out, depth - 1);
    }
    if out.len() == before {
        out.push_str(INTERMEDIATE);
    }
}

/** Visit one expression, printing what V8's printer prints for its kind. */
fn visit(expr: &Expr, out: &mut String, depth: usize) {
    match &expr.kind {
        ExprKind::Ident(name) => out.push_str(name),
        ExprKind::This => out.push_str("this"),
        ExprKind::NewTarget => out.push_str(".new.target"),
        ExprKind::Null => out.push_str("null"),
        ExprKind::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        ExprKind::Number(value) => out.push_str(&crate::v9::value::number_to_string(*value)),
        ExprKind::String(text) => {
            out.push('"');
            out.push_str(&text.lossy());
            out.push('"');
        }
        ExprKind::Regex(pattern, flags) => {
            out.push('/');
            out.push_str(pattern);
            out.push('/');
            out.push_str(flags);
        }
        ExprKind::Template(quasis, values) if values.is_empty() => {
            // A template without substitutions is a string literal by the time V8 prints it.
            out.push('"');
            out.push_str(
                &quasis
                    .first()
                    .cloned()
                    .flatten()
                    .map(|text| text.lossy())
                    .unwrap_or_default(),
            );
            out.push('"');
        }
        ExprKind::Template(_, values) => values.iter().for_each(|value| print(value, out, depth)),
        ExprKind::Array(elements) => array(elements, out, depth),
        ExprKind::Object(props) => object(props, out),
        ExprKind::Member(object, prop, optional) => member(object, prop, *optional, out, depth),
        // A finished optional chain is an intermediate value to V8; a call inside one prints.
        ExprKind::Chain(_) => {}
        ExprKind::Call(callee, ..) => {
            print(callee, out, depth);
            out.push_str("(...)");
        }
        ExprKind::Tagged(tag, ..) => {
            print(tag, out, depth);
            out.push_str("(...)");
        }
        ExprKind::Unary(operator, operand) => unary(*operator, operand, out, depth),
        ExprKind::Update(increment, prefix, operand) => {
            let token = if *increment { "++" } else { "--" };
            out.push('(');
            if *prefix {
                out.push_str(token);
            }
            print(operand, out, depth);
            if !*prefix {
                out.push_str(token);
            }
            out.push(')');
        }
        ExprKind::Binary(..) | ExprKind::Logical(..) => nary(expr, out, depth),
        ExprKind::Sequence(values) => sequence(values, out, depth),
        ExprKind::Assign(_, target, _) => pattern(target, out, depth),
        ExprKind::Conditional(..) => {
            for _ in 0..CONDITIONAL_PARTS {
                out.push_str(INTERMEDIATE);
            }
        }
        ExprKind::Import(specifier, _) => {
            out.push_str("import(");
            print(specifier, out, depth);
            out.push(')');
        }
        _ => {}
    }
}

/** `[a,b]`: elements joined by commas, holes empty, spreads as `(...x)`. */
fn array(elements: &[Element], out: &mut String, depth: usize) {
    out.push('[');
    for (index, element) in elements.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        match element {
            Element::Value(value) => print(value, out, depth),
            Element::Spread(value) => {
                out.push_str("(...");
                print(value, out, depth);
                out.push(')');
            }
            Element::Hole => {}
        }
    }
    out.push(']');
}

/** `{...}`: V8 prints every property value as an intermediate value, keys not at all. */
fn object(props: &[PropDef], out: &mut String) {
    out.push('{');
    for _ in props {
        out.push_str(INTERMEDIATE);
    }
    out.push('}');
}

/** `o.name`, `o[key]`, `o?.name`, `o?.[key]`; a string key prints after a dot whatever its text. */
fn member(object: &Expr, prop: &MemberProp, optional: bool, out: &mut String, depth: usize) {
    print(object, out, depth);
    if optional {
        out.push('?');
    }
    match prop {
        MemberProp::Name(name) => {
            out.push('.');
            out.push_str(name);
        }
        MemberProp::Computed(key) => match string_key(key) {
            Some(text) => {
                out.push('.');
                out.push_str(&text);
            }
            None => {
                out.push_str(if optional { ".[" } else { "[" });
                print(key, out, depth);
                out.push(']');
            }
        },
        MemberProp::Private(name) => {
            out.push_str(if optional { ".[#" } else { "[#" });
            out.push_str(name);
            out.push(']');
        }
    }
}

/** A prefix operator in parentheses, except the ones V8's parser folds into a literal. */
fn unary(operator: UnaryOp, operand: &Expr, out: &mut String, depth: usize) {
    if let Some(folded) = fold_unary(operator, operand) {
        out.push_str(&folded);
        return;
    }
    out.push('(');
    out.push_str(unary_token(operator));
    print(operand, out, depth);
    out.push(')');
}

/** `-1`, `+1`, `~1` and `!literal` are literals by the time V8 prints them. */
fn fold_unary(operator: UnaryOp, operand: &Expr) -> Option<String> {
    match (operator, &operand.kind) {
        (UnaryOp::Not, ExprKind::Number(value)) => {
            Some((*value == 0.0 || value.is_nan()).to_string())
        }
        (UnaryOp::Not, ExprKind::String(text)) => Some(text.is_empty().to_string()),
        (UnaryOp::Not, ExprKind::Null) => Some("true".into()),
        (UnaryOp::Not, ExprKind::Bool(value)) => Some((!value).to_string()),
        (UnaryOp::Minus, ExprKind::Number(value)) => {
            Some(crate::v9::value::number_to_string(-value))
        }
        (UnaryOp::Plus, ExprKind::Number(value)) => {
            Some(crate::v9::value::number_to_string(*value))
        }
        (UnaryOp::BitNot, ExprKind::Number(value)) => Some(crate::v9::value::number_to_string(
            f64::from(!to_int32(*value)),
        )),
        _ => None,
    }
}

/** ToInt32, for folding `~` on a number literal as the parser does. */
fn to_int32(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    (value.trunc().rem_euclid(TWO_POW_32) as u32) as i32
}

/** 2^32, the modulus of ToInt32. */
const TWO_POW_32: f64 = 4_294_967_296.0;

/** `(a op b op c)`: a chain of one operator prints flat, as V8's parser folds it into one node. */
fn nary(expr: &Expr, out: &mut String, depth: usize) {
    let (token, operands) = operands(expr);
    out.push('(');
    for (index, operand) in operands.iter().enumerate() {
        if index != 0 {
            out.push(' ');
            out.push_str(token);
            out.push(' ');
        }
        print(operand, out, depth);
    }
    out.push(')');
}

/** The operator token and, left to right, the operands of a chain of that operator. */
fn operands(expr: &Expr) -> (&'static str, Vec<&Expr>) {
    let (token, left, right) = match &expr.kind {
        ExprKind::Binary(operator, left, right) => (binary_token(*operator), left, right),
        ExprKind::Logical(operator, left, right) => (logical_token(*operator), left, right),
        _ => return ("", vec![expr]),
    };
    let mut operands = match operands(left) {
        (same, chain) if same == token => chain,
        _ => vec![left.as_ref()],
    };
    operands.push(right);
    (token, operands)
}

/** The text of a key that V8 prints after a dot: a string literal, or a template with nothing
to substitute, which its parser already made a string. */
fn string_key(key: &Expr) -> Option<String> {
    match &key.kind {
        ExprKind::String(text) => Some(text.lossy()),
        ExprKind::Template(quasis, values) if values.is_empty() => Some(
            quasis
                .first()
                .cloned()
                .flatten()
                .map(|text| text.lossy())
                .unwrap_or_default(),
        ),
        _ => None,
    }
}

/** `(a , b , c)`: the comma operator prints like any other binary operator. */
fn sequence(values: &[Expr], out: &mut String, depth: usize) {
    out.push('(');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            out.push_str(" , ");
        }
        print(value, out, depth);
    }
    out.push(')');
}

/** An assignment prints its target: a name, a member, or the literal shape of a pattern. */
fn pattern(target: &Pattern, out: &mut String, depth: usize) {
    match target {
        Pattern::Ident(name, _) => out.push_str(name),
        Pattern::Expr(member) => print(member, out, depth),
        Pattern::Object(props, ..) => {
            out.push('{');
            for _ in props {
                out.push_str(INTERMEDIATE);
            }
            out.push('}');
        }
        Pattern::Array(items, ..) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    out.push(',');
                }
                if let Some(item) = item {
                    pattern(item, out, depth);
                }
            }
            out.push(']');
        }
        Pattern::Default(inner, _) => pattern(inner, out, depth),
    }
}

/** The source token of a prefix operator, with the space V8 prints after a word. */
fn unary_token(operator: UnaryOp) -> &'static str {
    match operator {
        UnaryOp::Minus => "-",
        UnaryOp::Plus => "+",
        UnaryOp::Not => "!",
        UnaryOp::BitNot => "~",
        UnaryOp::Typeof => "typeof ",
        UnaryOp::Void => "void ",
        UnaryOp::Delete => "delete ",
    }
}

/** The source token of a binary operator. */
fn binary_token(operator: BinaryOp) -> &'static str {
    match operator {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::Exp => "**",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::StrictEq => "===",
        BinaryOp::StrictNe => "!==",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::In => "in",
        BinaryOp::InstanceOf => "instanceof",
    }
}

/** The source token of a logical operator. */
fn logical_token(operator: LogicalOp) -> &'static str {
    match operator {
        LogicalOp::And => "&&",
        LogicalOp::Or => "||",
        LogicalOp::Nullish => "??",
    }
}
