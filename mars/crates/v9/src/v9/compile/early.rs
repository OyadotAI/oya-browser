/**
 * The early errors code generation raises, checked over a deferred function's whole subtree when
 * it defers.
 *
 * A few early errors are found while emitting code rather than while parsing: conflicting
 * declarations, strict parameter rules, catch bindings, `with` in strict code, a duplicate
 * `__proto__` or a `{a = 1}` left in an object literal, and `delete` of a private field. They
 * make the whole script fail before it runs, even inside a function that is never called, so a
 * function that defers its compile still answers for them at load. This walk applies the same
 * checks, with the same functions, to every statement and expression the deferred compile will
 * later emit, nested functions included.
 */
use super::*;
use crate::v9::syntax::{
    Body, Class, Declarator, Element, ExprKind, ForHead, ForInit, MemberKind, MemberProp, Pattern,
    PropDef, PropKey, StmtKind, UnaryOp, VarKind,
};

/** Check a function literal: its parameters, its body and everything nested in it. */
pub(super) fn function(func: &Func) -> EngineResult<()> {
    let strict = func.strict;
    let mut params = Vec::new();
    for param in func.params.iter().chain(&func.rest) {
        scan::bound_names(param, &mut params);
        pattern(param, strict)?;
    }
    super::function::parameter_early_errors(func, &params)?;
    match &func.body {
        Body::Block(body) => {
            super::stmt::early_conflict(body, strict, true, &params)?;
            statements(body, strict)
        }
        Body::Expr(value) => expr(value, strict),
    }
}

/** Each statement of a list. */
fn statements(body: &[Stmt], strict: bool) -> EngineResult<()> {
    body.iter().try_for_each(|stmt| statement(stmt, strict))
}

/** A statement list with its own block scope, as `enter_block` checks it. */
fn block(body: &[Stmt], strict: bool) -> EngineResult<()> {
    super::stmt::early_conflict(body, strict, false, &[])?;
    statements(body, strict)
}

/** One statement. */
fn statement(stmt: &Stmt, strict: bool) -> EngineResult<()> {
    match &stmt.kind {
        StmtKind::Expr(value) | StmtKind::Throw(value) => expr(value, strict),
        StmtKind::Var(_, list) => declarators(list, strict),
        StmtKind::Function(func) => function(func),
        StmtKind::Class(class) => class_literal(class),
        StmtKind::Return(value) => optional(value.as_ref(), strict),
        StmtKind::If(test, yes, no) => {
            expr(test, strict)?;
            statement(yes, strict)?;
            no.as_deref().map_or(Ok(()), |no| statement(no, strict))
        }
        StmtKind::Block(body) => block(body, strict),
        StmtKind::For {
            init,
            test,
            update,
            body,
        } => {
            match init {
                Some(ForInit::Declare(kind, list)) => {
                    declarators(list, strict)?;
                    if *kind != VarKind::Var {
                        let mut names = Vec::new();
                        for declarator in list {
                            scan::bound_names(&declarator.target, &mut names);
                        }
                        super::stmt::head_conflict(stmt, &names, body)?;
                    }
                }
                Some(ForInit::Expr(value)) => expr(value, strict)?,
                None => {}
            }
            optional(test.as_ref(), strict)?;
            optional(update.as_ref(), strict)?;
            statement(body, strict)
        }
        StmtKind::ForIn(head, right, body) | StmtKind::ForOf(head, right, body, _) => {
            match head {
                ForHead::Declare(kind, target) => {
                    pattern(target, strict)?;
                    if *kind != VarKind::Var {
                        let mut names = Vec::new();
                        scan::bound_names(target, &mut names);
                        super::stmt::head_conflict(stmt, &names, body)?;
                    }
                }
                ForHead::Target(target) => pattern(target, strict)?,
            }
            expr(right, strict)?;
            statement(body, strict)
        }
        StmtKind::While(test, body) | StmtKind::DoWhile(body, test) => {
            expr(test, strict)?;
            statement(body, strict)
        }
        StmtKind::Try {
            block: body,
            param,
            handler,
            finalizer,
        } => {
            block(body, strict)?;
            if let Some(handler) = handler {
                if let Some(param) = param {
                    let mut names = Vec::new();
                    scan::bound_names(param, &mut names);
                    super::catch_bindings::validate(param, &names, handler)?;
                    pattern(param, strict)?;
                }
                block(handler, strict)?;
            }
            finalizer
                .as_deref()
                .map_or(Ok(()), |body| block(body, strict))
        }
        StmtKind::Switch(value, cases) => {
            expr(value, strict)?;
            for case in cases {
                optional(case.test.as_ref(), strict)?;
                statements(&case.body, strict)?;
            }
            Ok(())
        }
        StmtKind::Labeled(_, body) => statement(body, strict),
        StmtKind::With(..) if strict => Err(errors::syntax(
            stmt.pos.location(),
            "Strict mode code may not include a with statement",
        )),
        StmtKind::With(object, body) => {
            expr(object, strict)?;
            statement(body, strict)
        }
        StmtKind::Field(key, value) => {
            property_key(key, strict)?;
            optional(value.as_ref(), strict)
        }
        StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Empty => Ok(()),
    }
}

/** The targets and initializers of a declaration. */
fn declarators(list: &[Declarator], strict: bool) -> EngineResult<()> {
    for declarator in list {
        pattern(&declarator.target, strict)?;
        optional(declarator.init.as_ref(), strict)?;
    }
    Ok(())
}

/** An expression that may be absent. */
fn optional(value: Option<&Expr>, strict: bool) -> EngineResult<()> {
    value.map_or(Ok(()), |value| expr(value, strict))
}

/** Each expression of a list. */
fn all(values: &[Expr], strict: bool) -> EngineResult<()> {
    values.iter().try_for_each(|value| expr(value, strict))
}

/** Array elements and call arguments. */
fn elements(list: &[Element], strict: bool) -> EngineResult<()> {
    list.iter().try_for_each(|element| match element {
        Element::Value(value) | Element::Spread(value) => expr(value, strict),
        Element::Hole => Ok(()),
    })
}

/** One expression. */
fn expr(value: &Expr, strict: bool) -> EngineResult<()> {
    match &value.kind {
        ExprKind::Template(_, parts) => all(parts, strict),
        ExprKind::Tagged(tag, _, _, parts) => {
            expr(tag, strict)?;
            all(parts, strict)
        }
        #[cfg(feature = "conformance")]
        ExprKind::NativeIntrinsic(_, list) => elements(list, strict),
        ExprKind::Array(list) => elements(list, strict),
        ExprKind::Object(properties) => object_literal(properties, strict),
        ExprKind::Function(func) => function(func),
        ExprKind::Class(class) => class_literal(class),
        ExprKind::Unary(UnaryOp::Delete, operand)
            if matches!(operand.kind, ExprKind::Member(_, MemberProp::Private(_), _)) =>
        {
            Err(errors::syntax(
                operand.pos.location(),
                "Private fields can not be deleted",
            ))
        }
        ExprKind::Unary(_, operand)
        | ExprKind::Update(_, _, operand)
        | ExprKind::Chain(operand)
        | ExprKind::Await(operand)
        | ExprKind::PrivateIn(_, operand) => expr(operand, strict),
        ExprKind::Binary(_, left, right) | ExprKind::Logical(_, left, right) => {
            expr(left, strict)?;
            expr(right, strict)
        }
        ExprKind::Assign(_, target, value) => {
            pattern(target, strict)?;
            expr(value, strict)
        }
        ExprKind::Conditional(test, yes, no) => {
            expr(test, strict)?;
            expr(yes, strict)?;
            expr(no, strict)
        }
        ExprKind::Call(callee, list, _) | ExprKind::New(callee, list) => {
            expr(callee, strict)?;
            elements(list, strict)
        }
        ExprKind::Member(object, prop, _) => {
            expr(object, strict)?;
            match prop {
                MemberProp::Computed(key) => expr(key, strict),
                MemberProp::Name(_) | MemberProp::Private(_) => Ok(()),
            }
        }
        ExprKind::Sequence(list) => all(list, strict),
        ExprKind::Yield(operand, _) => optional(operand.as_deref(), strict),
        ExprKind::Import(specifier, options) => {
            expr(specifier, strict)?;
            optional(options.as_deref(), strict)
        }
        _ => Ok(()),
    }
}

/** An object literal left as an expression: the literal-only rules, then its parts. */
fn object_literal(properties: &[PropDef], strict: bool) -> EngineResult<()> {
    super::expr::literal_prototypes(properties)?;
    for property in properties {
        match property {
            PropDef::Value(key, value) => {
                property_key(key, strict)?;
                expr(value, strict)?;
            }
            PropDef::Shorthand(..) => {}
            PropDef::ShorthandDefault(_, pos, _) => {
                return Err(errors::syntax(
                    pos.location(),
                    "Invalid shorthand property initializer",
                ));
            }
            PropDef::Method(key, func)
            | PropDef::Getter(key, func)
            | PropDef::Setter(key, func) => {
                property_key(key, strict)?;
                function(func)?;
            }
            PropDef::Spread(value) => expr(value, strict)?,
        }
    }
    Ok(())
}

/** A computed key's expression. */
fn property_key(key: &PropKey, strict: bool) -> EngineResult<()> {
    match key {
        PropKey::Computed(value) => expr(value, strict),
        PropKey::Name(_) | PropKey::Private(_) => Ok(()),
    }
}

/** A destructuring or binding target: its computed keys and defaults are expressions. */
fn pattern(target: &Pattern, strict: bool) -> EngineResult<()> {
    match target {
        Pattern::Ident(..) => Ok(()),
        Pattern::Expr(value) => expr(value, strict),
        Pattern::Object(properties, rest, _) => {
            for property in properties {
                property_key(&property.key, strict)?;
                pattern(&property.value, strict)?;
            }
            rest.as_deref().map_or(Ok(()), |rest| pattern(rest, strict))
        }
        Pattern::Array(items, rest, _) => {
            for item in items.iter().flatten() {
                pattern(item, strict)?;
            }
            rest.as_deref().map_or(Ok(()), |rest| pattern(rest, strict))
        }
        Pattern::Default(target, value) => {
            pattern(target, strict)?;
            expr(value, strict)
        }
    }
}

/** A class: its heritage, constructor, members and initializers. Class code is strict. */
fn class_literal(class: &Class) -> EngineResult<()> {
    optional(class.superclass.as_ref(), true)?;
    function(&class.constructor)?;
    for member in &class.members {
        property_key(&member.key, true)?;
        match &member.kind {
            MemberKind::Method(func) | MemberKind::Getter(func) | MemberKind::Setter(func) => {
                function(func)?
            }
            MemberKind::AutoAccessor(_) => {}
        }
    }
    for func in class.fields.iter().chain(&class.statics) {
        function(func)?;
    }
    Ok(())
}
