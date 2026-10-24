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
        self.function_tail(pos, site(pos, SITE_SOURCE), name, display, flags)
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
        self.function_tail(
            pos,
            site(pos, SITE_SOURCE),
            None,
            Name::from(display.as_str()),
            flags,
        )
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
            function: true,
            generator: flags.generator,
            asynchronous: flags.asynchronous,
            breakable: false,
            continuable: false,
            method: flags.method || flags.constructor || saved.0.method && flags.arrow,
            derived: flags.derived,
        };
        let result = self.params_and_body();
        let strict = self.strict;
        self.context = saved.0;
        self.strict = saved.1;
        self.labels = saved.2;
        self.unnest();
        let (params, rest, body) = result?;
        Ok(Rc::new(Func {
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
    fn params_and_body(&mut self) -> EngineResult<(Vec<Pattern>, Option<Pattern>, Vec<Stmt>)> {
        let (params, rest) = self.parameters()?;
        self.expect("{")?;
        self.strict |= self.directives();
        let body = self.statement_list(false)?;
        self.expect("}")?;
        Ok((params, rest, body))
    }
    /** `( a, b = 1, {c}, ...rest )` */
    fn parameters(&mut self) -> EngineResult<(Vec<Pattern>, Option<Pattern>)> {
        self.expect("(")?;
        let mut params = Vec::new();
        let mut rest = None;
        while !self.eat(")") {
            if self.eat("...") {
                rest = Some(self.binding_target()?);
                self.expect(")")?;
                break;
            }
            params.push(self.binding_element()?);
            if params.len() > crate::constants::FUNCTION_PARAMETERS {
                return Err(errors::located(
                    EngineError::limit(
                        "V9 function parameters",
                        crate::constants::FUNCTION_PARAMETERS,
                    ),
                    self.peek().location,
                    "parse",
                ));
            }
            if !self.at(")") {
                self.expect(",")?;
            }
        }
        Ok((params, rest))
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
            function: true,
            generator: false,
            asynchronous: is_async,
            breakable: false,
            continuable: false,
            method: outer.method,
            derived: outer.derived,
        };
        let result = self.arrow_parts(simple, no_in);
        let strict = self.strict;
        self.context = saved.0;
        self.strict = saved.1;
        self.labels = saved.2;
        self.unnest();
        let (params, rest, body) = result?;
        let func = Rc::new(Func {
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
            self.strict |= self.directives();
            let body = self.statement_list(false)?;
            self.expect("}")?;
            Body::Block(body)
        } else {
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
        let display = name.clone().unwrap_or_else(|| Name::from(""));
        let constructor = match constructor {
            Some(func) => {
                *func.display.borrow_mut() = display;
                func
            }
            None => Rc::new(Func {
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
        Ok(Rc::new(Class {
            name,
            superclass,
            constructor,
            members,
            fields: initializer(pos, SITE_FIELDS, fields),
            statics: initializer(pos, SITE_STATICS, statics),
            pos,
        }))
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
                statics.push(Stmt {
                    kind: StmtKind::Block(body),
                    pos,
                });
                return Ok(());
            }
        }
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
        if self.at("(") {
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
            let func = self.method(pos, &key, is_async, generator, accessor)?;
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
            Some(self.field_initializer()?)
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
    /** A field initializer sees `this` and `super.x` but not `arguments`, `await` or `yield`. */
    fn field_initializer(&mut self) -> EngineResult<Expr> {
        let saved = self.context;
        self.context = Context {
            function: true,
            method: true,
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
            ..Context::default()
        };
        let body = self.block();
        self.context = saved.0;
        self.labels = saved.1;
        body
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

/** A synthesized class initializer running field definitions and static blocks in order. */
fn initializer(pos: Pos, tag: u32, body: Vec<Stmt>) -> Option<Rc<Func>> {
    if body.is_empty() {
        return None;
    }
    Some(Rc::new(Func {
        site: site(pos, tag),
        name: None,
        display: RefCell::new(Name::from("")),
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
