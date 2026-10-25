/** Expression grammar: precedence climbing, member chains, literals and destructuring covers. */
use super::stmt::infer_name;
use super::*;
use crate::v9::value::JsString;

impl Parser {
    /** A full Expression, including the comma operator. `no_in` excludes `in` for for-heads. */
    pub(super) fn expression(&mut self, no_in: bool) -> EngineResult<Expr> {
        let pos = self.pos();
        let first = self.assignment(no_in)?;
        if !self.at(",") {
            return Ok(first);
        }
        let mut items = vec![first];
        while self.eat(",") {
            items.push(self.assignment(no_in)?);
        }
        self.node(ExprKind::Sequence(items), pos)
    }
    /** AssignmentExpression: arrows, yield, conditional and every assignment operator. */
    pub(super) fn assignment(&mut self, no_in: bool) -> EngineResult<Expr> {
        self.enter_expression(|parser| parser.assignment_level(no_in))
    }
    /** One assignment level; `assignment` bounds the recursion. */
    fn assignment_level(&mut self, no_in: bool) -> EngineResult<Expr> {
        if let Some(arrow) = self.arrow(no_in)? {
            return Ok(arrow);
        }
        if self.at_name("yield") && self.context.generator {
            return self.yield_expression(no_in);
        }
        let pos = self.pos();
        let target = self.conditional(no_in)?;
        let operator = match self.punct() {
            Some("=") => AssignOp::Assign,
            Some(spelling) => match compound(spelling) {
                Some(operator) => operator,
                None => return Ok(target),
            },
            None => return Ok(target),
        };
        self.next();
        let pattern = if operator == AssignOp::Assign {
            self.pattern_of(target)?
        } else {
            self.simple_target(target)?
        };
        let mut value = self.assignment(no_in)?;
        if let Pattern::Ident(name, _) = &pattern {
            infer_name(&mut value, name);
        }
        self.node(
            ExprKind::Assign(operator, Box::new(pattern), Box::new(value)),
            pos,
        )
    }
    /** `yield`, `yield value` and `yield* iterable`. */
    fn yield_expression(&mut self, no_in: bool) -> EngineResult<Expr> {
        let pos = self.pos();
        if self.context.parameters {
            return Err(self.error_at(pos, "Yield expression not allowed in formal parameter"));
        }
        self.next();
        let delegate = !self.peek().line_terminator_before && self.eat("*");
        let ends = self.peek().line_terminator_before && !delegate
            || matches!(self.peek().kind, Kind::End)
            || matches!(self.punct(), Some(")" | "]" | "}" | "," | ";" | ":"))
            || self.at_name("in") && !delegate;
        let argument = if ends && !delegate {
            None
        } else {
            Some(Box::new(self.assignment(no_in)?))
        };
        self.node(ExprKind::Yield(argument, delegate), pos)
    }
    /** `test ? yes : no` */
    fn conditional(&mut self, no_in: bool) -> EngineResult<Expr> {
        let pos = self.pos();
        let test = self.binary(0, no_in)?;
        if !self.eat("?") {
            return Ok(test);
        }
        let yes = self.assignment(false)?;
        self.expect(":")?;
        let no = self.assignment(no_in)?;
        self.node(
            ExprKind::Conditional(Box::new(test), Box::new(yes), Box::new(no)),
            pos,
        )
    }
    /** Binary operators by precedence; `**` is right-associative. */
    fn binary(&mut self, minimum: u8, no_in: bool) -> EngineResult<Expr> {
        self.enter(|parser| parser.binary_level(minimum, no_in))
    }
    /** One precedence-climbing level. */
    fn binary_level(&mut self, minimum: u8, no_in: bool) -> EngineResult<Expr> {
        let pos = self.pos();
        let mut left = if matches!(self.peek().kind, Kind::Private(_)) {
            self.private_in(no_in)?
        } else {
            self.unary()?
        };
        loop {
            let Some((precedence, operator)) = self.binary_operator(no_in) else {
                return Ok(left);
            };
            if precedence < minimum {
                return Ok(left);
            }
            self.next();
            let right_minimum = if precedence == EXPONENT {
                precedence
            } else {
                precedence + 1
            };
            let right = self.binary(right_minimum, no_in)?;
            let kind = match operator {
                Binary::Plain(op) => ExprKind::Binary(op, Box::new(left), Box::new(right)),
                Binary::Logical(op) => ExprKind::Logical(op, Box::new(left), Box::new(right)),
            };
            left = self.node(kind, pos)?;
        }
    }
    /** `#name in object`, the private brand check. */
    fn private_in(&mut self, no_in: bool) -> EngineResult<Expr> {
        let pos = self.pos();
        let Kind::Private(name) = self.next().kind else {
            return Err(self.unexpected());
        };
        if no_in || !self.eat_name("in") {
            return Err(self.unexpected());
        }
        let name = Name::from(name.as_str());
        self.use_private(&name, pos)?;
        let object = self.binary(9, no_in)?;
        self.node(ExprKind::PrivateIn(name, Box::new(object)), pos)
    }
    /** The binary operator at the cursor with its precedence. */
    fn binary_operator(&self, no_in: bool) -> Option<(u8, Binary)> {
        use BinaryOp::*;
        if let Kind::Name(name) = &self.peek().kind {
            return match name.as_str() {
                "instanceof" => Some((8, Binary::Plain(InstanceOf))),
                "in" if !no_in => Some((8, Binary::Plain(In))),
                _ => None,
            };
        }
        let operator = match self.punct()? {
            "??" => (1, Binary::Logical(LogicalOp::Nullish)),
            "||" => (2, Binary::Logical(LogicalOp::Or)),
            "&&" => (3, Binary::Logical(LogicalOp::And)),
            "|" => (4, Binary::Plain(BitOr)),
            "^" => (5, Binary::Plain(BitXor)),
            "&" => (6, Binary::Plain(BitAnd)),
            "==" => (7, Binary::Plain(Eq)),
            "!=" => (7, Binary::Plain(Ne)),
            "===" => (7, Binary::Plain(StrictEq)),
            "!==" => (7, Binary::Plain(StrictNe)),
            "<" => (8, Binary::Plain(Lt)),
            ">" => (8, Binary::Plain(Gt)),
            "<=" => (8, Binary::Plain(Le)),
            ">=" => (8, Binary::Plain(Ge)),
            "<<" => (9, Binary::Plain(Shl)),
            ">>" => (9, Binary::Plain(Shr)),
            ">>>" => (9, Binary::Plain(UShr)),
            "+" => (10, Binary::Plain(Add)),
            "-" => (10, Binary::Plain(Sub)),
            "*" => (11, Binary::Plain(Mul)),
            "/" => (11, Binary::Plain(Div)),
            "%" => (11, Binary::Plain(Mod)),
            "**" => (EXPONENT, Binary::Plain(Exp)),
            _ => return None,
        };
        Some(operator)
    }
    /** Prefix operators, `await`, and prefix updates. */
    pub(super) fn unary(&mut self) -> EngineResult<Expr> {
        self.enter(|parser| parser.unary_level())
    }
    /** One unary level. */
    fn unary_level(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        if self.at("++") || self.at("--") {
            let increment = self.at("++");
            self.next();
            let operand = self.unary()?;
            let target = self.simple_target(operand)?;
            let target = match target {
                Pattern::Ident(name, pos) => self.node(ExprKind::Ident(name), pos)?,
                Pattern::Expr(expr) => *expr,
                _ => return Err(self.error_at(pos, "Invalid left-hand side in prefix operation")),
            };
            return self.node(ExprKind::Update(increment, true, Box::new(target)), pos);
        }
        let operator = match (&self.peek().kind, self.punct()) {
            (_, Some("-")) => Some(UnaryOp::Minus),
            (_, Some("+")) => Some(UnaryOp::Plus),
            (_, Some("!")) => Some(UnaryOp::Not),
            (_, Some("~")) => Some(UnaryOp::BitNot),
            (Kind::Name(name), _) if name == "typeof" => Some(UnaryOp::Typeof),
            (Kind::Name(name), _) if name == "void" => Some(UnaryOp::Void),
            (Kind::Name(name), _) if name == "delete" => Some(UnaryOp::Delete),
            _ => None,
        };
        if let Some(operator) = operator {
            self.next();
            let operand = self.unary()?;
            if operator == UnaryOp::Delete
                && self.strict
                && matches!(operand.kind, ExprKind::Ident(_))
            {
                return Err(
                    self.error_at(pos, "Delete of an unqualified identifier in strict mode.")
                );
            }
            if operator == UnaryOp::Delete && deletes_private(&operand) {
                return Err(self.error_at(pos, "Private fields can not be deleted"));
            }
            if self.at("**") {
                return Err(self.unexpected());
            }
            return self.node(ExprKind::Unary(operator, Box::new(operand)), pos);
        }
        if self.at_name("await") && self.context.asynchronous {
            if self.context.parameters {
                return Err(self.error_at(pos, "Illegal await-expression in formal parameters"));
            }
            self.next();
            let operand = self.unary()?;
            return self.node(ExprKind::Await(Box::new(operand)), pos);
        }
        self.postfix()
    }
    /** Postfix `++` and `--` with no line break before them. */
    fn postfix(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        let value = self.left_hand_side()?;
        if (self.at("++") || self.at("--")) && !self.peek().line_terminator_before {
            let increment = self.at("++");
            self.next();
            let target = match self.simple_target(value)? {
                Pattern::Ident(name, pos) => self.node(ExprKind::Ident(name), pos)?,
                Pattern::Expr(expr) => *expr,
                _ => return Err(self.error_at(pos, "Invalid left-hand side in postfix operation")),
            };
            return self.node(ExprKind::Update(increment, false, Box::new(target)), pos);
        }
        Ok(value)
    }
    /** `new`, member access, calls, optional chains and tagged templates. */
    fn left_hand_side(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        let base = if self.at_name("new") {
            self.new_expression()?
        } else if self.at_name("super") {
            self.super_expression()?
        } else if self.at_name("import") {
            self.import_expression()?
        } else {
            self.primary()?
        };
        self.chain(base, pos, true)
    }
    /** `import(...)` loads a module from any script; `import.meta` is a module's own: anywhere
    else (a classic script, `Function` or eval code) it is an early SyntaxError, as is an escaped
    `meta`. */
    fn import_expression(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        self.next();
        if self.eat(".") {
            if !self.at_name("meta") || self.module.is_none() {
                return Err(self.error_at(pos, "import.meta is only valid in a module"));
            }
            self.next();
            return self.node(ExprKind::ImportMeta, pos);
        }
        if !self.at("(") {
            return Err(self.unexpected());
        }
        let mut arguments = self.arguments()?.into_iter();
        let mut value = || match arguments.next() {
            Some(Element::Value(value)) => Ok(Some(Box::new(value))),
            None => Ok(None),
            Some(_) => Err(self.error_at(pos, "import() requires a specifier")),
        };
        let (Some(specifier), options) = (value()?, value()?) else {
            return Err(self.error_at(pos, "import() requires a specifier"));
        };
        if value()?.is_some() {
            return Err(self.error_at(pos, "import() requires a specifier"));
        }
        self.node(ExprKind::Import(specifier, options), pos)
    }
    /** `new Callee(args)`, `new Callee`, and `new.target`. */
    fn new_expression(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        self.next();
        if self.eat(".") {
            let name = self.property_name()?;
            if &*name != "target" || !self.context.function {
                return Err(self.error_at(pos, "new.target expression is not allowed here"));
            }
            return self.node(ExprKind::NewTarget, pos);
        }
        let callee_pos = self.pos();
        let callee = if self.at_name("new") {
            self.new_expression()?
        } else if self.at_name("super") {
            self.super_expression()?
        } else if self.at_name("import") {
            // `new import.meta` is a MemberExpression; `new import(...)` is an early error.
            let callee = self.import_expression()?;
            if !matches!(callee.kind, ExprKind::ImportMeta) {
                return Err(self.error_at(callee_pos, "Cannot use new with import"));
            }
            callee
        } else {
            self.primary()?
        };
        let callee = self.chain(callee, callee_pos, false)?;
        let arguments = if self.at("(") {
            self.arguments()?
        } else {
            Vec::new()
        };
        self.node(ExprKind::New(Box::new(callee), arguments), pos)
    }
    /** `super.x`, `super[x]` and `super(...)`. */
    fn super_expression(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        self.next();
        if self.at("(") && !self.context.derived
            || (self.at(".") || self.at("[")) && !self.context.method
            || !(self.at("(") || self.at(".") || self.at("["))
        {
            return Err(self.error_at(pos, "'super' keyword unexpected here"));
        }
        self.node(ExprKind::Super, pos)
    }
    /** Member, call and template continuations; `calls` is false inside a `new` callee. */
    fn chain(&mut self, mut base: Expr, pos: Pos, calls: bool) -> EngineResult<Expr> {
        let mut optional = false;
        loop {
            match self.punct() {
                Some(".") => {
                    self.next();
                    let property = self.member_name()?;
                    base = self.node(ExprKind::Member(Box::new(base), property, false), pos)?;
                }
                Some("?.") => {
                    if !calls {
                        return Err(
                            self.error_at(self.pos(), "Invalid optional chain from new expression")
                        );
                    }
                    self.next();
                    optional = true;
                    base = if self.at("(") {
                        let arguments = self.arguments()?;
                        self.node(ExprKind::Call(Box::new(base), arguments, true), pos)?
                    } else if self.eat("[") {
                        let key = self.expression(false)?;
                        self.expect("]")?;
                        self.node(
                            ExprKind::Member(
                                Box::new(base),
                                MemberProp::Computed(Box::new(key)),
                                true,
                            ),
                            pos,
                        )?
                    } else {
                        let property = self.member_name()?;
                        self.node(ExprKind::Member(Box::new(base), property, true), pos)?
                    };
                }
                Some("[") => {
                    self.next();
                    let key = self.expression(false)?;
                    self.expect("]")?;
                    base = self.node(
                        ExprKind::Member(
                            Box::new(base),
                            MemberProp::Computed(Box::new(key)),
                            false,
                        ),
                        pos,
                    )?;
                }
                Some("(") if calls => {
                    let arguments = self.arguments()?;
                    base = self.node(ExprKind::Call(Box::new(base), arguments, false), pos)?;
                }
                _ if matches!(self.peek().kind, Kind::Template { head: true, .. }) => {
                    if optional {
                        return Err(
                            self.error_at(self.pos(), "Invalid tagged template on optional chain")
                        );
                    }
                    let (quasis, raws, expressions) = self.template_parts()?;
                    base = self.node(
                        ExprKind::Tagged(Box::new(base), quasis, raws, expressions),
                        pos,
                    )?;
                }
                _ => break,
            }
        }
        if optional {
            base = self.node(ExprKind::Chain(Box::new(base)), pos)?;
        }
        Ok(base)
    }
    /** A dotted name: an identifier name or a `#private` name. */
    fn member_name(&mut self) -> EngineResult<MemberProp> {
        if let Kind::Private(name) = &self.peek().kind {
            let name = Name::from(name.as_str());
            let pos = self.pos();
            self.next();
            self.use_private(&name, pos)?;
            return Ok(MemberProp::Private(name));
        }
        Ok(MemberProp::Name(self.property_name()?))
    }
    /** `( arguments )`, with spread. */
    pub(super) fn arguments(&mut self) -> EngineResult<Vec<Element>> {
        self.expect("(")?;
        let mut list = Vec::new();
        while !self.eat(")") {
            if self.eat("...") {
                list.push(Element::Spread(self.assignment(false)?));
            } else {
                list.push(Element::Value(self.assignment(false)?));
            }
            if !self.at(")") {
                self.expect(",")?;
            }
        }
        Ok(list)
    }
    /** Literals, names, groups, functions, classes, arrays, objects and templates. */
    fn primary(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        let token = self.peek().clone();
        let kind = match token.kind {
            Kind::Number(value) => {
                self.next();
                ExprKind::Number(value)
            }
            Kind::String(value) => {
                self.next();
                ExprKind::String(value)
            }
            Kind::Opaque(feature) => {
                self.next();
                ExprKind::Opaque(feature)
            }
            Kind::BigInt(value) => {
                self.next();
                ExprKind::BigInt(value)
            }
            Kind::Regex { pattern, flags } => {
                self.next();
                ExprKind::Regex(Name::from(pattern.as_str()), Name::from(flags.as_str()))
            }
            Kind::Template { head: true, .. } => {
                let (quasis, _, expressions) = self.template_parts()?;
                ExprKind::Template(quasis, expressions)
            }
            Kind::Symbol('(') => {
                self.next();
                let value = self.expression(false)?;
                self.expect(")")?;
                return Ok(value);
            }
            Kind::Symbol('[') => return self.array_literal(),
            Kind::Symbol('{') => return self.object_literal(),
            // An escaped name is never a keyword, so it takes the identifier arm.
            Kind::Name(name) => match if token.escaped { "" } else { name.as_str() } {
                "this" => {
                    self.next();
                    ExprKind::This
                }
                "null" => {
                    self.next();
                    ExprKind::Null
                }
                "true" | "false" => {
                    self.next();
                    ExprKind::Bool(name == "true")
                }
                "function" => {
                    self.next();
                    return self
                        .function_rest(pos, false, false)
                        .and_then(|func| self.node(ExprKind::Function(func), pos));
                }
                "async"
                    if matches!(&self.peek_at(1).kind, Kind::Name(n) if n == "function")
                        && !self.peek_at(1).line_terminator_before =>
                {
                    self.next();
                    self.next();
                    return self
                        .function_rest(pos, true, false)
                        .and_then(|func| self.node(ExprKind::Function(func), pos));
                }
                "class" => {
                    let class = self.class(false)?;
                    ExprKind::Class(class)
                }
                _ => {
                    let name = self.identifier()?;
                    self.check_arguments(&name, pos)?;
                    ExprKind::Ident(name)
                }
            },
            _ => return Err(self.unexpected()),
        };
        self.node(kind, pos)
    }
    /** Template chunks (cooked and raw) and substitutions; an uncookable chunk is None. */
    #[allow(clippy::type_complexity)]
    fn template_parts(
        &mut self,
    ) -> EngineResult<(Vec<Option<JsString>>, Vec<JsString>, Vec<Expr>)> {
        let mut quasis = Vec::new();
        let mut raws = Vec::new();
        let mut expressions = Vec::new();
        let Kind::Template {
            cooked, raw, tail, ..
        } = self.next().kind
        else {
            return Err(self.unexpected());
        };
        quasis.push(cooked);
        raws.push(raw);
        let mut tail = tail;
        while !tail {
            expressions.push(self.expression(false)?);
            match self.next().kind {
                Kind::Template {
                    cooked,
                    raw,
                    head: false,
                    tail: last,
                } => {
                    quasis.push(cooked);
                    raws.push(raw);
                    tail = last;
                }
                _ => {
                    self.position -= 1;
                    return Err(self.unexpected());
                }
            }
        }
        Ok((quasis, raws, expressions))
    }
    /** `[a, , ...b]` */
    fn array_literal(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        self.expect("[")?;
        let mut elements = Vec::new();
        while !self.eat("]") {
            if self.eat(",") {
                elements.push(Element::Hole);
                continue;
            }
            if self.eat("...") {
                elements.push(Element::Spread(self.assignment(false)?));
            } else {
                elements.push(Element::Value(self.assignment(false)?));
            }
            if !self.at("]") {
                self.expect(",")?;
            }
        }
        self.node(ExprKind::Array(elements), pos)
    }
    /** `{a, b: c, [k]: v, m() {}, get x() {}, ...rest}` */
    fn object_literal(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        self.expect("{")?;
        let mut properties = Vec::new();
        while !self.eat("}") {
            properties.push(self.property_definition()?);
            if !self.at("}") {
                self.expect(",")?;
            }
        }
        self.node(ExprKind::Object(properties), pos)
    }
    /** One object-literal member. */
    fn property_definition(&mut self) -> EngineResult<PropDef> {
        if self.eat("...") {
            return Ok(PropDef::Spread(self.assignment(false)?));
        }
        let pos = self.pos();
        let is_async = self.at_name("async")
            && !self.next_ends_key()
            && !self.peek_at(1).line_terminator_before;
        if is_async {
            self.next();
        }
        let generator = self.eat("*");
        let accessor = if !is_async
            && !generator
            && (self.at_name("get") || self.at_name("set"))
            && !self.next_ends_key()
        {
            let getter = self.at_name("get");
            self.next();
            Some(getter)
        } else {
            None
        };
        let shorthand = matches!(self.peek().kind, Kind::Name(_))
            && accessor.is_none()
            && !is_async
            && !generator
            && matches!(self.peek_at(1).kind, Kind::Symbol(',' | '}' | '='));
        if shorthand {
            let name = self.identifier()?;
            self.check_arguments(&name, pos)?;
            if self.eat("=") {
                let mut value = self.assignment(false)?;
                infer_name(&mut value, &name);
                return Ok(PropDef::ShorthandDefault(name, pos, Box::new(value)));
            }
            return Ok(PropDef::Shorthand(name, pos));
        }
        let key = self.property_key()?;
        if matches!(key, PropKey::Private(_)) {
            return Err(self.error_at(pos, "unexpected private name"));
        }
        if let Some(getter) = accessor {
            let func = self.method(pos, &key, false, false, Some(getter))?;
            return Ok(if getter {
                PropDef::Getter(key, func)
            } else {
                PropDef::Setter(key, func)
            });
        }
        if self.at("(") {
            let func = self.method(pos, &key, is_async, generator, None)?;
            return Ok(PropDef::Method(key, func));
        }
        if is_async || generator {
            return Err(self.unexpected());
        }
        self.expect(":")?;
        let mut value = self.assignment(false)?;
        if let PropKey::Name(name) = &key {
            infer_name(&mut value, name);
        }
        Ok(PropDef::Value(key, value))
    }
    /** True when the token after the cursor ends a property key, so `get` is itself the key. */
    fn next_ends_key(&self) -> bool {
        matches!(
            self.peek_at(1).kind,
            Kind::Symbol(',' | ':' | '(' | '}' | '=')
        )
    }
    /** An identifier, string, number, computed or private property key. */
    pub(super) fn property_key(&mut self) -> EngineResult<PropKey> {
        let token = self.next();
        Ok(match token.kind {
            Kind::Name(name) => PropKey::Name(Name::from(name.as_str())),
            Kind::String(value) => PropKey::Name(Name::from(
                value
                    .to_dom()
                    .map_err(|error| errors::located(error, token.location, "parse"))?
                    .as_str(),
            )),
            Kind::Number(value) => PropKey::Name(Name::from(
                crate::v9::value::number_to_string(value).as_str(),
            )),
            Kind::Private(name) => PropKey::Private(Name::from(name.as_str())),
            Kind::Symbol('[') => {
                let key = self.assignment(false)?;
                self.expect("]")?;
                PropKey::Computed(Box::new(key))
            }
            _ => {
                self.position -= 1;
                return Err(self.unexpected());
            }
        })
    }
    /** Turn a parsed expression into an assignment or binding pattern (the cover grammar). */
    pub(super) fn pattern_of(&mut self, value: Expr) -> EngineResult<Pattern> {
        let pos = value.pos;
        match value.kind {
            ExprKind::Ident(name) => {
                self.check_assignable(&name, pos)?;
                Ok(Pattern::Ident(name, pos))
            }
            ExprKind::Member(..) => Ok(Pattern::Expr(Box::new(value))),
            ExprKind::Array(elements) => {
                let mut items = Vec::new();
                let mut rest = None;
                let count = elements.len();
                for (index, element) in elements.into_iter().enumerate() {
                    match element {
                        Element::Hole => items.push(None),
                        Element::Value(value) => items.push(Some(self.pattern_of(value)?)),
                        Element::Spread(value)
                            if index + 1 == count
                                && !matches!(value.kind, ExprKind::Assign(..)) =>
                        {
                            rest = Some(Box::new(self.pattern_of(value)?));
                        }
                        Element::Spread(_) => {
                            return Err(self.error_at(pos, "Rest element must be last element"));
                        }
                    }
                }
                Ok(Pattern::Array(items, rest, pos))
            }
            ExprKind::Object(properties) => {
                let mut items = Vec::new();
                let mut rest = None;
                let count = properties.len();
                for (index, property) in properties.into_iter().enumerate() {
                    match property {
                        PropDef::Value(key, value) => items.push(PatProp {
                            key,
                            value: self.pattern_of(value)?,
                        }),
                        PropDef::Shorthand(name, pos) => {
                            self.check_assignable(&name, pos)?;
                            items.push(PatProp {
                                key: PropKey::Name(name.clone()),
                                value: Pattern::Ident(name, pos),
                            })
                        }
                        PropDef::ShorthandDefault(name, pos, value) => {
                            self.check_assignable(&name, pos)?;
                            items.push(PatProp {
                                key: PropKey::Name(name.clone()),
                                value: Pattern::Default(Box::new(Pattern::Ident(name, pos)), value),
                            })
                        }
                        PropDef::Spread(value) if index + 1 == count => {
                            rest = Some(Box::new(self.pattern_of(value)?));
                        }
                        _ => {
                            return Err(
                                self.error_at(pos, "Invalid destructuring assignment target")
                            );
                        }
                    }
                }
                Ok(Pattern::Object(items, rest, pos))
            }
            ExprKind::Assign(AssignOp::Assign, target, value) => {
                Ok(Pattern::Default(target, value))
            }
            _ => Err(self.error_at(pos, "Invalid left-hand side in assignment")),
        }
    }
    /** Compound assignment and updates need a plain name or member target. */
    fn simple_target(&mut self, value: Expr) -> EngineResult<Pattern> {
        let pos = value.pos;
        match value.kind {
            ExprKind::Ident(name) => {
                self.check_assignable(&name, pos)?;
                Ok(Pattern::Ident(name, pos))
            }
            ExprKind::Member(..) => Ok(Pattern::Expr(Box::new(value))),
            _ => Err(self.error_at(pos, "Invalid left-hand side in assignment")),
        }
    }
    /** Strict code cannot assign `eval` or `arguments`. */
    fn check_assignable(&self, name: &str, pos: Pos) -> EngineResult<()> {
        if self.strict && (name == "eval" || name == "arguments") {
            return Err(self.error_at(pos, "Unexpected eval or arguments in strict mode"));
        }
        Ok(())
    }
    /** If the cursor starts an arrow function, parse it. */
    fn arrow(&mut self, no_in: bool) -> EngineResult<Option<Expr>> {
        let pos = self.pos();
        let is_async = self.at_name("async")
            && !self.peek_at(1).line_terminator_before
            && (matches!(self.peek_at(1).kind, Kind::Symbol('('))
                && self.arrow_after(self.position + 1)
                || matches!(self.peek_at(1).kind, Kind::Name(_))
                    && matches!(self.peek_at(2).kind, Kind::Arrow));
        let offset = usize::from(is_async);
        let simple = matches!(&self.peek_at(offset).kind, Kind::Name(name) if !reserved(name, self.strict) || name == "yield" || name == "await")
            && matches!(self.peek_at(offset + 1).kind, Kind::Arrow)
            && !self.peek_at(offset + 1).line_terminator_before;
        let parenthesized = matches!(self.peek_at(offset).kind, Kind::Symbol('('))
            && self.arrow_after(self.position + offset);
        if !simple && !parenthesized {
            return Ok(None);
        }
        if is_async {
            self.next();
        }
        self.arrow_function(pos, is_async, simple, no_in).map(Some)
    }
    /** True if the parenthesis at `index` closes right before `=>` on the same line. */
    fn arrow_after(&self, index: usize) -> bool {
        let Some(close) = self
            .closers
            .get(index)
            .copied()
            .filter(|close| *close != NO_CLOSER)
        else {
            return false;
        };
        self.tokens
            .get(close as usize + 1)
            .is_some_and(|token| matches!(token.kind, Kind::Arrow) && !token.line_terminator_before)
    }
}

/** `delete x.#y` (also inside an optional chain) is an early error: private names never go away. */
fn deletes_private(operand: &Expr) -> bool {
    match &operand.kind {
        ExprKind::Member(_, MemberProp::Private(_), _) => true,
        ExprKind::Chain(inner) => deletes_private(inner),
        _ => false,
    }
}

/** `**` binds tighter than every other binary operator and associates to the right. */
const EXPONENT: u8 = 12;

/** Plain and short-circuiting binary operators. */
enum Binary {
    Plain(BinaryOp),
    Logical(LogicalOp),
}

/** Compound assignment spellings. */
fn compound(spelling: &str) -> Option<AssignOp> {
    use BinaryOp::*;
    Some(match spelling {
        "+=" => AssignOp::Binary(Add),
        "-=" => AssignOp::Binary(Sub),
        "*=" => AssignOp::Binary(Mul),
        "/=" => AssignOp::Binary(Div),
        "%=" => AssignOp::Binary(Mod),
        "**=" => AssignOp::Binary(Exp),
        "<<=" => AssignOp::Binary(Shl),
        ">>=" => AssignOp::Binary(Shr),
        ">>>=" => AssignOp::Binary(UShr),
        "&=" => AssignOp::Binary(BitAnd),
        "|=" => AssignOp::Binary(BitOr),
        "^=" => AssignOp::Binary(BitXor),
        "&&=" => AssignOp::Logical(LogicalOp::And),
        "||=" => AssignOp::Logical(LogicalOp::Or),
        "??=" => AssignOp::Logical(LogicalOp::Nullish),
        _ => return None,
    })
}
