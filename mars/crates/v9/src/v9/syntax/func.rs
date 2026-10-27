/** Function, arrow, method and class grammar, plus binding patterns. */
use super::stmt::infer_name;
use super::*;
use std::cell::RefCell;
use std::rc::Rc;

impl Parser {
    /** After `function`: optional `*`, a name (required for declarations), parameters and body. */
    pub(super) fn function_rest(
        &mut self,
        pos: Pos,
        is_async: bool,
        declaration: bool,
    ) -> EngineResult<Rc<Func>> {
        let generator = self.eat("*");
        let source_available = self.pos().offset.saturating_sub(pos.offset) <= u32::from(u16::MAX);
        let name = if self.at("(") {
            if declaration {
                return Err(self.unexpected());
            }
            None
        } else {
            let saved = self.context;
            // A function expression's own name follows its own generator and async status.
            if !declaration {
                self.context.generator = generator;
                self.context.asynchronous = is_async;
                self.context.static_block = false;
            }
            let name = self.identifier();
            self.context = saved;
            Some(name?)
        };
        let flags = FuncFlags {
            asynchronous: is_async,
            generator,
            ..FuncFlags::default()
        };
        let display = name.clone().unwrap_or_else(|| Name::from(""));
        let function = self.function_tail(pos, site(pos, SITE_SOURCE), name, display, flags)?;
        // The compact prefix offset follows V8's source-availability boundary.
        if !source_available {
            *function.source_span.borrow_mut() = None;
        }
        Ok(function)
    }
    /** An object or class method, getter or setter after its key. */
    pub(super) fn method(
        &mut self,
        pos: Pos,
        key: &PropKey,
        is_async: bool,
        generator: bool,
        accessor: Option<bool>,
    ) -> EngineResult<Rc<Func>> {
        let flags = FuncFlags {
            asynchronous: is_async,
            generator,
            method: true,
            getter: accessor == Some(true),
            setter: accessor == Some(false),
            ..FuncFlags::default()
        };
        let base = key_name(key);
        let display = match accessor {
            Some(true) => format!("get {base}"),
            Some(false) => format!("set {base}"),
            None => base,
        };
        let func = self.function_tail(
            pos,
            site(pos, SITE_SOURCE),
            None,
            Name::from(display.as_str()),
            flags,
        )?;
        // A getter takes no parameters; a setter exactly one, which is not a rest parameter.
        let arity = match accessor {
            Some(true) => func.params.is_empty() && func.rest.is_none(),
            Some(false) => func.params.len() == 1 && func.rest.is_none(),
            None => true,
        };
        if !arity {
            return Err(self.error_at(
                pos,
                "Getter must not have parameters; setter must have exactly one",
            ));
        }
        Ok(func)
    }
    /** Parameters and a block body under a fresh function context. */
    fn function_tail(
        &mut self,
        pos: Pos,
        site: u32,
        name: Option<Name>,
        display: Name,
        flags: FuncFlags,
    ) -> EngineResult<Rc<Func>> {
        self.nest(pos)?;
        let saved = (self.context, self.strict, std::mem::take(&mut self.labels));
        self.context = Context {
            using_allowed: true,
            function: true,
            generator: flags.generator,
            asynchronous: flags.asynchronous,
            breakable: false,
            continuable: false,
            method: flags.method || flags.constructor || saved.0.method && flags.arrow,
            derived: flags.derived,
            ..Context::default()
        };
        let result = self.params_and_body(flags.method || flags.constructor);
        let strict = self.strict;
        self.context = saved.0;
        self.strict = saved.1;
        self.labels = saved.2;
        self.unnest();
        let (params, rest, body) = result?;
        if strict
            && name
                .as_ref()
                .is_some_and(|name| matches!(&**name, "eval" | "arguments") || reserved(name, true))
        {
            return Err(self.error_at(pos, "Unexpected function name in strict mode"));
        }
        Ok(Rc::new(Func {
            source_span: RefCell::new(Some((pos.offset, self.consumed_end()))),
            site,
            name,
            display: RefCell::new(display),
            length: length(&params),
            params,
            rest,
            body: Body::Block(body),
            flags,
            strict,
            pos,
        }))
    }
    /** `(params) { body }`; a body directive makes the function strict. */
    fn params_and_body(
        &mut self,
        unique: bool,
    ) -> EngineResult<(Vec<Pattern>, Option<Pattern>, Vec<Stmt>)> {
        let pos = self.pos();
        let (params, rest) = self.parameters()?;
        self.expect("{")?;
        self.body_directives(pos, &params, rest.as_ref(), unique)?;
        let body = self.statement_list(false)?;
        self.expect("}")?;
        Ok((params, rest, body))
    }
    /** Read the body's directive prologue, then apply the parameter early errors: a `"use strict"`
    body needs a simple parameter list, and strict code re-checks the names it already bound. */
    fn body_directives(
        &mut self,
        pos: Pos,
        params: &[Pattern],
        rest: Option<&Pattern>,
        unique: bool,
    ) -> EngineResult<()> {
        let directive = self.directives();
        let simple = rest.is_none() && params.iter().all(|p| matches!(p, Pattern::Ident(..)));
        if directive && !simple {
            return Err(self.error_at(
                pos,
                "Illegal 'use strict' directive in function with non-simple parameter list",
            ));
        }
        self.strict |= directive;
        self.check_params(pos, params, rest, unique)
    }
    /** Duplicate parameter names are an error unless the function is sloppy with a simple list
    (`unique` marks arrows and methods, which never allow them); strict code also forbids
    `eval`, `arguments` and the strict reserved words as parameter names. */
    fn check_params(
        &self,
        pos: Pos,
        params: &[Pattern],
        rest: Option<&Pattern>,
        unique: bool,
    ) -> EngineResult<()> {
        let mut names = Vec::new();
        for param in params.iter().chain(rest) {
            bound_names(param, &mut names);
        }
        if self.strict
            && names
                .iter()
                .any(|name| matches!(&**name, "eval" | "arguments") || reserved(name, true))
        {
            return Err(self.error_at(pos, "Unexpected eval or arguments in strict mode"));
        }
        let simple = rest.is_none() && params.iter().all(|p| matches!(p, Pattern::Ident(..)));
        let strict_names = self.strict || unique || !simple;
        let mut seen = std::collections::HashSet::new();
        if strict_names && names.iter().any(|name| !seen.insert(name)) {
            return Err(self.error_at(pos, "Duplicate parameter name not allowed in this context"));
        }
        Ok(())
    }
    /** `( a, b = 1, {c}, ...rest )` */
    fn parameters(&mut self) -> EngineResult<(Vec<Pattern>, Option<Pattern>)> {
        let outer = std::mem::replace(&mut self.context.parameters, true);
        let result = self.parameter_list();
        self.context.parameters = outer;
        result
    }
    /** The list itself; `parameters` marks the context while it is read. */
    fn parameter_list(&mut self) -> EngineResult<(Vec<Pattern>, Option<Pattern>)> {
        self.expect("(")?;
        let mut params = Vec::new();
        let mut rest = None;
        while !self.eat(")") {
            if self.eat("...") {
                rest = Some(self.binding_target()?);
                self.check_parameter_count(params.len() + 1)?;
                self.expect(")")?;
                break;
            }
            params.push(self.binding_element()?);
            self.check_parameter_count(params.len())?;
            if !self.at(")") {
                self.expect(",")?;
            }
        }
        Ok((params, rest))
    }
    /** Native arity includes rest parameters independently of the observable function length. */
    fn check_parameter_count(&self, count: usize) -> EngineResult<()> {
        if count > crate::constants::FUNCTION_PARAMETERS {
            return Err(self.error_at(self.pos(), "Too many parameters in function definition"));
        }
        Ok(())
    }
    /** An arrow function; `simple` means a single unparenthesized parameter. */
    pub(super) fn arrow_function(
        &mut self,
        pos: Pos,
        is_async: bool,
        simple: bool,
        no_in: bool,
    ) -> EngineResult<Expr> {
        self.nest(pos)?;
        let outer = self.context;
        let saved = (self.context, self.strict, std::mem::take(&mut self.labels));
        self.context = Context {
            using_allowed: true,
            function: true,
            eval_new_target: false,
            generator: false,
            asynchronous: is_async,
            breakable: false,
            continuable: false,
            method: outer.method,
            derived: outer.derived,
            no_arguments: outer.no_arguments,
            static_block: false,
            parameters: false,
        };
        let result = self.arrow_parts(simple, no_in);
        let strict = self.strict;
        self.context = saved.0;
        self.strict = saved.1;
        self.labels = saved.2;
        self.unnest();
        let (params, rest, body) = result?;
        let func = Rc::new(Func {
            source_span: RefCell::new(Some((pos.offset, self.consumed_end()))),
            site: site(pos, SITE_SOURCE),
            name: None,
            display: RefCell::new(Name::from("")),
            length: length(&params),
            params,
            rest,
            body,
            flags: FuncFlags {
                arrow: true,
                asynchronous: is_async,
                ..FuncFlags::default()
            },
            strict,
            pos,
        });
        self.node(ExprKind::Function(func), pos)
    }
    /** Arrow parameters, `=>`, and a block or concise body. */
    fn arrow_parts(
        &mut self,
        simple: bool,
        no_in: bool,
    ) -> EngineResult<(Vec<Pattern>, Option<Pattern>, Body)> {
        let pos = self.pos();
        let (params, rest) = if simple {
            let pos = self.pos();
            let name = self.identifier()?;
            (vec![Pattern::Ident(name, pos)], None)
        } else {
            self.parameters()?
        };
        self.expect("=>")?;
        let body = if self.at("{") {
            self.next();
            self.body_directives(pos, &params, rest.as_ref(), true)?;
            let body = self.statement_list(false)?;
            self.expect("}")?;
            Body::Block(body)
        } else {
            self.check_params(pos, &params, rest.as_ref(), true)?;
            // A concise body is a return statement in disguise, so it costs one.
            self.count_statement(self.pos())?;
            Body::Expr(Box::new(self.assignment(no_in)?))
        };
        Ok((params, rest, body))
    }
    /** A binding name or a destructuring binding pattern. */
    pub(super) fn binding_target(&mut self) -> EngineResult<Pattern> {
        self.enter(|parser| parser.binding_target_level())
    }
    /** One binding-pattern level. */
    fn binding_target_level(&mut self) -> EngineResult<Pattern> {
        let pos = self.pos();
        if self.eat("[") {
            let mut items = Vec::new();
            let mut rest = None;
            while !self.eat("]") {
                if self.eat(",") {
                    items.push(None);
                    continue;
                }
                if self.eat("...") {
                    rest = Some(Box::new(self.binding_target()?));
                    self.expect("]")?;
                    break;
                }
                items.push(Some(self.binding_element()?));
                if !self.at("]") {
                    self.expect(",")?;
                }
            }
            return Ok(Pattern::Array(items, rest, pos));
        }
        if self.eat("{") {
            let mut items = Vec::new();
            let mut rest = None;
            while !self.eat("}") {
                if self.eat("...") {
                    rest = Some(Box::new(self.binding_target()?));
                    self.expect("}")?;
                    break;
                }
                let key_pos = self.pos();
                let shorthand = matches!(self.peek().kind, Kind::Name(_))
                    && matches!(self.peek_at(1).kind, Kind::Symbol(',' | '}' | '='));
                let (key, value) = if shorthand {
                    let name = self.identifier()?;
                    (PropKey::Name(name.clone()), Pattern::Ident(name, key_pos))
                } else {
                    let key = self.property_key()?;
                    if matches!(key, PropKey::Private(_)) {
                        return Err(self.error_at(key_pos, "unexpected private name"));
                    }
                    self.expect(":")?;
                    (key, self.binding_target()?)
                };
                let value = self.with_default(value)?;
                items.push(PatProp { key, value });
                if !self.at("}") {
                    self.expect(",")?;
                }
            }
            return Ok(Pattern::Object(items, rest, pos));
        }
        let name = self.identifier()?;
        if self.strict && (&*name == "eval" || &*name == "arguments") {
            return Err(self.error_at(pos, "Unexpected eval or arguments in strict mode"));
        }
        Ok(Pattern::Ident(name, pos))
    }
    /** A binding target with an optional `= default`. */
    fn binding_element(&mut self) -> EngineResult<Pattern> {
        let target = self.binding_target()?;
        self.with_default(target)
    }
    /** Attach `= default` if present. */
    fn with_default(&mut self, target: Pattern) -> EngineResult<Pattern> {
        if !self.eat("=") {
            return Ok(target);
        }
        let mut value = self.assignment(false)?;
        if let Pattern::Ident(name, _) = &target {
            infer_name(&mut value, name);
        }
        Ok(Pattern::Default(Box::new(target), Box::new(value)))
    }
    /** A class declaration or expression; class bodies are always strict. */
    pub(super) fn class(&mut self, declaration: bool) -> EngineResult<Rc<Class>> {
        let pos = self.pos();
        self.expect_name("class")?;
        let saved_strict = self.strict;
        self.strict = true;
        let result = self.class_rest(pos, declaration);
        self.strict = saved_strict;
        result
    }
    /** Name, heritage and body. */
    fn class_rest(&mut self, pos: Pos, declaration: bool) -> EngineResult<Rc<Class>> {
        let name = if self.at_identifier() && !self.at_name("extends") {
            Some(self.identifier()?)
        } else if declaration {
            return Err(self.unexpected());
        } else {
            None
        };
        let superclass = if self.eat_name("extends") {
            Some(self.left_hand_side_for_class()?)
        } else {
            None
        };
        let derived = superclass.is_some();
        self.privates.push(PrivateScope::default());
        let body = self.class_body(derived);
        let closed = self.close_privates();
        let (members, constructor, fields, statics) = body?;
        let privates = closed?;
        let display = name.clone().unwrap_or_else(|| Name::from(""));
        let constructor = match constructor {
            Some(func) => {
                *func.display.borrow_mut() = display;
                func
            }
            None => Rc::new(Func {
                source_span: RefCell::new(None),
                site: site(pos, SITE_CONSTRUCTOR),
                name: None,
                display: RefCell::new(display),
                params: Vec::new(),
                rest: None,
                body: Body::Block(Vec::new()),
                flags: FuncFlags {
                    constructor: true,
                    derived,
                    method: true,
                    ..FuncFlags::default()
                },
                strict: true,
                pos,
                length: 0,
            }),
        };
        *constructor.source_span.borrow_mut() = Some((pos.offset, self.consumed_end()));
        Ok(Rc::new(Class {
            name,
            superclass,
            constructor,
            members,
            fields: initializer(pos, SITE_FIELDS, fields),
            statics: initializer(pos, SITE_STATICS, statics),
            privates,
            pos,
        }))
    }
    /** `{ members }`: methods, the constructor, instance fields and static elements. */
    #[allow(clippy::type_complexity)]
    fn class_body(
        &mut self,
        derived: bool,
    ) -> EngineResult<(Vec<ClassMember>, Option<Rc<Func>>, Vec<Stmt>, Vec<Stmt>)> {
        self.expect("{")?;
        let mut members = Vec::new();
        let mut constructor = None;
        let mut fields = Vec::new();
        let mut statics = Vec::new();
        while !self.eat("}") {
            if self.eat(";") {
                continue;
            }
            self.class_member(
                derived,
                &mut members,
                &mut constructor,
                &mut fields,
                &mut statics,
            )?;
        }
        Ok((members, constructor, fields, statics))
    }
    /** The heritage expression is a LeftHandSideExpression. */
    fn left_hand_side_for_class(&mut self) -> EngineResult<Expr> {
        self.enter(|parser| parser.unary())
    }
    /** One class element. */
    fn class_member(
        &mut self,
        derived: bool,
        members: &mut Vec<ClassMember>,
        constructor: &mut Option<Rc<Func>>,
        fields: &mut Vec<Stmt>,
        statics: &mut Vec<Stmt>,
    ) -> EngineResult<()> {
        let pos = self.pos();
        let is_static = self.at_name("static")
            && !matches!(self.peek_at(1).kind, Kind::Symbol('(' | '=' | ';' | '}'));
        if is_static {
            self.next();
            if self.at("{") {
                let body = self.static_block(pos)?;
                if let Some(mut function) = initializer(pos, SITE_SOURCE, body) {
                    // Each static block owns its vars while retaining lexical this and super.
                    Rc::get_mut(&mut function)
                        .expect("new static block")
                        .flags
                        .arrow = true;
                    let callee = self.node(ExprKind::Function(function), pos)?;
                    let call =
                        self.node(ExprKind::Call(Box::new(callee), Vec::new(), false), pos)?;
                    statics.push(Stmt {
                        kind: StmtKind::Expr(call),
                        pos,
                    });
                }
                return Ok(());
            }
        }
        let auto_accessor = self.at_name("accessor")
            && !self.peek_at(1).line_terminator_before
            && !matches!(self.peek_at(1).kind, Kind::Symbol('(' | '=' | ';' | '}'));
        if auto_accessor {
            self.next();
            return self.auto_accessor_member(pos, is_static, members, fields, statics);
        }
        let method_pos = self.pos();
        let is_async = self.at_name("async")
            && !matches!(self.peek_at(1).kind, Kind::Symbol('(' | '=' | ';' | '}'))
            && !self.peek_at(1).line_terminator_before;
        if is_async {
            self.next();
        }
        let generator = self.eat("*");
        let accessor = if !is_async
            && !generator
            && (self.at_name("get") || self.at_name("set"))
            && !matches!(self.peek_at(1).kind, Kind::Symbol('(' | '=' | ';' | '}'))
        {
            let getter = self.at_name("get");
            self.next();
            Some(getter)
        } else {
            None
        };
        let key = self.property_key()?;
        let method = self.at("(");
        if let Some(reason) = class_key_error(&key, is_static, method, accessor.is_some()) {
            return Err(self.error_at(pos, reason));
        }
        if let PropKey::Private(name) = &key {
            let kind = match accessor {
                Some(true) => PrivateKind::Getter(is_static),
                Some(false) => PrivateKind::Setter(is_static),
                None => PrivateKind::Other,
            };
            self.declare_private(name, kind, pos)?;
        }
        if method {
            let is_constructor = !is_static
                && accessor.is_none()
                && matches!(&key, PropKey::Name(name) if &**name == "constructor");
            if is_constructor {
                if is_async || generator || constructor.is_some() {
                    return Err(self.error_at(pos, "A class may only have one constructor"));
                }
                let flags = FuncFlags {
                    constructor: true,
                    derived,
                    method: true,
                    ..FuncFlags::default()
                };
                let func =
                    self.function_tail(pos, site(pos, SITE_SOURCE), None, Name::from(""), flags)?;
                *constructor = Some(func);
                return Ok(());
            }
            let func = self.method(method_pos, &key, is_async, generator, accessor)?;
            let kind = match accessor {
                Some(true) => MemberKind::Getter(func),
                Some(false) => MemberKind::Setter(func),
                None => MemberKind::Method(func),
            };
            members.push(ClassMember {
                key,
                is_static,
                kind,
                pos,
            });
            return Ok(());
        }
        if is_async || generator || accessor.is_some() {
            return Err(self.unexpected());
        }
        let value = if self.eat("=") {
            let mut value = self.field_initializer()?;
            match &key {
                PropKey::Name(name) => infer_name(&mut value, name),
                PropKey::Private(name) => infer_name(&mut value, &format!("#{name}")),
                PropKey::Computed(_) => {}
            }
            Some(value)
        } else {
            None
        };
        self.semicolon()?;
        let stmt = Stmt {
            kind: StmtKind::Field(key, value),
            pos,
        };
        if is_static {
            statics.push(stmt);
        } else {
            fields.push(stmt);
        }
        Ok(())
    }
    /** Auto-accessors declare a private backing field and an independently placed real accessor pair. */
    fn auto_accessor_member(
        &mut self,
        pos: Pos,
        is_static: bool,
        members: &mut Vec<ClassMember>,
        fields: &mut Vec<Stmt>,
        statics: &mut Vec<Stmt>,
    ) -> EngineResult<()> {
        let key = self.property_key()?;
        if let Some(reason) = class_key_error(&key, is_static, false, true) {
            return Err(self.error_at(pos, reason));
        }
        if let PropKey::Private(name) = &key {
            self.declare_private(name, PrivateKind::Other, pos)?;
        }
        let backing = Name::from(format!("%accessor{}", pos.offset).as_str());
        self.declare_private(&backing, PrivateKind::Other, pos)?;
        let value = if self.eat("=") {
            let mut value = self.field_initializer()?;
            match &key {
                PropKey::Name(name) => infer_name(&mut value, name),
                PropKey::Private(name) => infer_name(&mut value, &format!("#{name}")),
                PropKey::Computed(_) => {}
            }
            Some(value)
        } else {
            None
        };
        self.semicolon()?;
        members.push(ClassMember {
            key,
            is_static,
            kind: MemberKind::AutoAccessor(backing.clone()),
            pos,
        });
        let field = Stmt {
            kind: StmtKind::Field(PropKey::Private(backing), value),
            pos,
        };
        if is_static {
            statics.push(field);
        } else {
            fields.push(field);
        }
        Ok(())
    }
    /** A field initializer sees `this` and `super.x` but not `arguments`, `await` or `yield`. */
    fn field_initializer(&mut self) -> EngineResult<Expr> {
        let saved = self.context;
        self.context = Context {
            using_allowed: true,
            function: true,
            method: true,
            no_arguments: true,
            ..Context::default()
        };
        let value = self.assignment(false);
        self.context = saved;
        value
    }
    /** `static { ... }` */
    fn static_block(&mut self, _pos: Pos) -> EngineResult<Vec<Stmt>> {
        let saved = (self.context, std::mem::take(&mut self.labels));
        self.context = Context {
            function: false,
            method: true,
            no_arguments: true,
            static_block: true,
            ..Context::default()
        };
        let body = self.block();
        self.context = saved.0;
        self.labels = saved.1;
        body
    }
}

/** Every name a binding pattern binds, in source order. */
pub(super) fn bound_names(pattern: &Pattern, names: &mut Vec<Name>) {
    match pattern {
        Pattern::Ident(name, _) => names.push(name.clone()),
        Pattern::Expr(_) => {}
        Pattern::Object(items, rest, _) => {
            for item in items {
                bound_names(&item.value, names);
            }
            if let Some(rest) = rest {
                bound_names(rest, names);
            }
        }
        Pattern::Array(items, rest, _) => {
            for item in items.iter().flatten() {
                bound_names(item, names);
            }
            if let Some(rest) = rest {
                bound_names(rest, names);
            }
        }
        Pattern::Default(target, _) => bound_names(target, names),
    }
}

/** Names a class element may not take: no field or accessor named `constructor`, no static
member named `prototype`, no static field named `constructor`, and no `#constructor`. */
fn class_key_error(
    key: &PropKey,
    is_static: bool,
    method: bool,
    accessor: bool,
) -> Option<&'static str> {
    let name = match key {
        PropKey::Name(name) => &**name,
        PropKey::Private(name) if &**name == "constructor" => {
            return Some("Classes may not have a private field named '#constructor'");
        }
        _ => return None,
    };
    match name {
        "prototype" if is_static => {
            Some("Classes may not have a static property named 'prototype'")
        }
        "constructor" if !method => Some("Classes may not have a field named 'constructor'"),
        "constructor" if accessor && !is_static => Some("Class constructor may not be an accessor"),
        _ => None,
    }
}

/** The code identity of a function starting at `pos`. */
pub(in crate::v9) fn site(pos: Pos, tag: u32) -> u32 {
    pos.offset * SITE_TAGS + tag
}

/** Parameters before the first default or rest count toward `length`. */
fn length(params: &[Pattern]) -> u32 {
    params
        .iter()
        .take_while(|param| !matches!(param, Pattern::Default(..)))
        .count() as u32
}

/** The display name a property key gives a method. */
fn key_name(key: &PropKey) -> String {
    match key {
        PropKey::Name(name) => name.to_string(),
        PropKey::Private(name) => format!("#{name}"),
        PropKey::Computed(_) => String::new(),
    }
}

/** Only the class's own field and static initializers carry V8's synthetic frame names. */
fn initializer_name(tag: u32) -> &'static str {
    match tag {
        SITE_FIELDS => INSTANCE_INITIALIZER_NAME,
        SITE_STATICS => STATIC_INITIALIZER_NAME,
        _ => "",
    }
}

/** A synthesized class initializer running field definitions and static blocks in order. */
fn initializer(pos: Pos, tag: u32, body: Vec<Stmt>) -> Option<Rc<Func>> {
    if body.is_empty() {
        return None;
    }
    Some(Rc::new(Func {
        source_span: RefCell::new(None),
        site: site(pos, tag),
        name: None,
        display: RefCell::new(Name::from(initializer_name(tag))),
        params: Vec::new(),
        rest: None,
        body: Body::Block(body),
        flags: FuncFlags {
            method: true,
            fields: true,
            ..FuncFlags::default()
        },
        strict: true,
        pos,
        length: 0,
    }))
}
