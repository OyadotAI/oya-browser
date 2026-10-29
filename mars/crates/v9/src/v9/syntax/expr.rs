/** Expression grammar: precedence climbing, member chains, literals and destructuring covers. */
use super::stmt::infer_name;
use super::*;
use crate::v9::value::JsString;

/** V8 tiering and feedback hints that V9, which has no optimizer, accepts with any arguments and
answers `undefined`, exactly as V8 does under `--no-turbofan --no-maglev`. Without them every upstream
language test that warms a function up before asserting its result would stay excluded. */
#[cfg(feature = "conformance")]
const OPTIMIZER_HINTS: &[&str] = &[
    "PrepareFunctionForOptimization",
    "OptimizeFunctionOnNextCall",
    "OptimizeMaglevOnNextCall",
    "NeverOptimizeFunction",
    "OptimizeOsr",
    "BaselineOsr",
    "DeoptimizeFunction",
    "DeoptimizeNow",
    "_DeoptimizeNow",
    "ClearFunctionFeedback",
    "EnsureFeedbackVectorForFunction",
    "TurbofanStaticAssert",
    "AssertEscapeAnalysisElided",
    "AssertPeeled",
    "DisableOptimizationFinalization",
    "FinalizeOptimization",
    "WaitForBackgroundOptimization",
    "WasmTierUpFunction",
    "SetAllocationTimeout",
    "SimulateNewspaceFull",
    "DebugToggleBlockCoverage",
];
/** The precedence of `<`, `>`, `instanceof` and `in` in `binary_operator`'s table; a brand check
`#name in object` is only grammatical where an expression of that level may start. */
const RELATIONAL_PRECEDENCE: u8 = 8;
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
        let start = self.position;
        let target = self.conditional(no_in)?;
        let operator = match self.punct() {
            Some("=") => Some(AssignOp::Assign),
            Some(spelling) => compound(spelling),
            None => None,
        };
        let Some(operator) = operator else {
            self.check_arrow_head(&target, start)?;
            return Ok(target);
        };
        self.next();
        let pattern = if operator == AssignOp::Assign {
            self.legacy_target(target)?
        } else {
            if matches!(operator, AssignOp::Logical(_)) && matches!(target.kind, ExprKind::Call(..))
            {
                return Err(self.error_at(pos, INVALID_ASSIGNMENT));
            }
            self.simple_target(target, INVALID_ASSIGNMENT)?
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
    /** An `=>` after an expression that `arrow` did not take as a head is V8's malformed
    parameter list, unless the expression is a name or starts with a parenthesis: those are
    heads a line break kept from `=>`, and the arrow is then simply the unexpected token. */
    fn check_arrow_head(&self, target: &Expr, start: usize) -> EngineResult<()> {
        // `async (...)` on one line is a head too; only a line break kept it from the arrow.
        let async_head = matches!(target.kind, ExprKind::Call(..))
            && self.at_index_name(start, "async")
            && matches!(self.tokens[start + 1].kind, Kind::Symbol('('))
            && !self.tokens[start + 1].line_terminator_before;
        let plain = matches!(target.kind, ExprKind::Ident(_))
            || matches!(self.tokens[start].kind, Kind::Symbol('('))
            || async_head;
        if self.at("=>") && !plain {
            return Err(self.error_at(target.pos, "Malformed arrow function parameter list"));
        }
        Ok(())
    }
    /** True when the token at `index` is this unescaped name. */
    fn at_index_name(&self, index: usize, name: &str) -> bool {
        let token = &self.tokens[index];
        !token.escaped && matches!(&token.kind, Kind::Name(found) if found == name)
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
    /** One precedence-climbing level. A `#name in` brand check is a RelationalExpression, so it
    may open this level only when the level admits relational operators: as the operand of a
    tighter operator (`0 << #x in {}`) a bare private name is the early error the spec requires. */
    fn binary_level(&mut self, minimum: u8, no_in: bool) -> EngineResult<Expr> {
        let pos = self.pos();
        let private_in_allowed = minimum <= RELATIONAL_PRECEDENCE;
        let mut left = if private_in_allowed && matches!(self.peek().kind, Kind::Private(_)) {
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
            if let Binary::Logical(operator) = &operator {
                self.check_logical_mixture(*operator, &left, minimum)?;
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
    /** CoalesceExpression and LogicalORExpression are separate grammar branches unless a
    parenthesis joins them, so the operator that mixes them is the unexpected token: a `??`
    after an unparenthesized `||` or `&&` operand, or a `||` or `&&` met while a `??` operand
    is being read (only that operand is read at NULLISH_OPERAND). */
    fn check_logical_mixture(
        &self,
        operator: LogicalOp,
        left: &Expr,
        minimum: u8,
    ) -> EngineResult<()> {
        let nullish = operator == LogicalOp::Nullish;
        let mixed_left = matches!(&left.kind, ExprKind::Logical(inner, ..)
            if (*inner == LogicalOp::Nullish) != nullish)
            && !self.parenthesized_logical.contains(&left.pos.offset);
        if mixed_left || !nullish && minimum == NULLISH_OPERAND {
            return Err(self.unexpected());
        }
        Ok(())
    }
    /** `#name in object`, the private brand check. */
    fn private_in(&mut self, no_in: bool) -> EngineResult<Expr> {
        let pos = self.pos();
        let Kind::Private(name) = self.next().kind else {
            return Err(self.unexpected());
        };
        if no_in || !self.eat_name("in") {
            // The name itself is what V8 refuses: a `#name` stands nowhere but before `in`.
            self.position -= 1;
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
            "??" => (NULLISH, Binary::Logical(LogicalOp::Nullish)),
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
            let target = self.simple_target(operand, INVALID_PREFIX)?;
            let target = match target {
                Pattern::Ident(name, pos) => self.node(ExprKind::Ident(name), pos)?,
                Pattern::Expr(expr) => *expr,
                _ => return Err(self.error_at(pos, INVALID_PREFIX)),
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
                return Err(self.error_at(pos, UNARY_EXPONENT));
            }
            return self.node(ExprKind::Unary(operator, Box::new(operand)), pos);
        }
        if self.at_name("await") && self.context.asynchronous {
            if self.context.parameters {
                return Err(self.await_in_parameters(pos));
            }
            self.next();
            let operand = self.unary()?;
            if self.at("**") {
                return Err(self.error_at(pos, UNARY_EXPONENT));
            }
            return self.node(ExprKind::Await(Box::new(operand)), pos);
        }
        self.postfix()
    }
    /** An `await` expression inside async parameters. V8 reads a sloppy `async (...)` head as a
    call first, where `await` is a plain name: the name is the error unless an operand follows
    it, which then leaves the call's argument list unterminated. */
    fn await_in_parameters(&self, pos: Pos) -> EngineError {
        if !self.context.sloppy_async_head {
            return self.error_at(pos, AWAIT_IN_PARAMETERS);
        }
        let ends = matches!(self.peek_at(1).kind, Kind::End)
            || matches!(
                self.peek_at(1).kind,
                Kind::Symbol(')' | ',' | '=' | ']' | '}')
            );
        if ends {
            self.error_at(pos, AWAIT_BINDING)
        } else {
            self.error_at(
                Pos::from_location(self.peek_at(1).location),
                UNTERMINATED_ARGUMENTS,
            )
        }
    }
    /** Postfix `++` and `--` with no line break before them. */
    fn postfix(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        let value = self.left_hand_side()?;
        if (self.at("++") || self.at("--")) && !self.peek().line_terminator_before {
            let increment = self.at("++");
            self.next();
            let target = match self.simple_target(value, INVALID_POSTFIX)? {
                Pattern::Ident(name, pos) => self.node(ExprKind::Ident(name), pos)?,
                Pattern::Expr(expr) => *expr,
                _ => return Err(self.error_at(pos, INVALID_POSTFIX)),
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
            if !self.at_name("meta") {
                return Err(self.unexpected());
            }
            if self.module.is_none() {
                return Err(self.error_at(pos, "Cannot use 'import.meta' outside a module"));
            }
            self.next();
            return self.node(ExprKind::ImportMeta, pos);
        }
        if !self.at("(") {
            return Err(self.error_at(pos, IMPORT_OUTSIDE_MODULE));
        }
        self.expect("(")?;
        let specifier = self.import_argument()?;
        let Some(specifier) = specifier else {
            return Err(self.error_at(pos, "import() requires a specifier"));
        };
        let options = if self.at(")") {
            None
        } else {
            self.import_argument()?
        };
        self.expect(")")?;
        self.node(ExprKind::Import(specifier, options), pos)
    }
    /** One `import()` argument with its optional trailing comma: V8 reads at most two, then
    expects the closing parenthesis, so a third is simply the unexpected token it starts with. */
    fn import_argument(&mut self) -> EngineResult<Option<Box<Expr>>> {
        if self.at(")") {
            return Ok(None);
        }
        if self.at("...") {
            return Err(self.unexpected());
        }
        let value = self.assignment(false)?;
        if !self.at(")") && !self.eat(",") {
            return Err(self.unexpected());
        }
        Ok(Some(Box::new(value)))
    }
    /** `new Callee(args)`, `new Callee`, and `new.target`. The `target` keyword is a terminal
    symbol, so an escaped spelling is not it; and only a function body, a class element or eval
    code inside one may ask for it (an arrow asks for its enclosing one). */
    fn new_expression(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        self.next();
        if self.eat(".") {
            if !matches!(&self.peek().kind, Kind::Name(name) if name == "target") {
                return Err(self.unexpected());
            }
            if self.peek().escaped {
                return Err(self.error_at(pos, "'new.target' must not contain escaped characters"));
            }
            self.next();
            if !self.context.new_target {
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
                    // SuperProperty names a public member: `super.#x` has no brand to check.
                    if matches!(base.kind, ExprKind::Super)
                        && matches!(property, MemberProp::Private(_))
                    {
                        return Err(self.error_at(pos, "Unexpected private field"));
                    }
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
                    if matches!(self.peek().kind, Kind::Template { head: true, .. }) {
                        return Err(
                            self.error_at(self.pos(), "Invalid tagged template on optional chain")
                        );
                    }
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
                    let (quasis, raws, expressions) = self.template_parts(true)?;
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
            if list.len() > crate::constants::FUNCTION_PARAMETERS {
                return Err(self.error_at(self.pos(), "Too many arguments in function call"));
            }
            if !self.at(")") && !self.eat(",") {
                return Err(self.error_at(self.pos(), UNTERMINATED_ARGUMENTS));
            }
        }
        Ok(list)
    }
    /** A literal whose pattern does not compile is an early error even if the literal never runs,
    as ECMA-262 requires (the compiler also refuses a `\k<name>` that names no group). */
    fn regex_literal_check(&self, pos: Pos, pattern: &str, flags: &str) -> EngineResult<()> {
        let units: Vec<u16> = pattern.encode_utf16().collect();
        // Boundary assertions are accepted here; the runtime still refuses them where the environment disables them.
        match crate::v9::regexp::compile(&units, flags, true) {
            // Only ECMAScript early errors throw here; V9's own gaps wait for the runtime, so real pages still parse.
            Err(error) if !error.is_engine_gap() => {
                Err(self.regex_error(pos, pattern, flags, &error.0))
            }
            _ => Ok(()),
        }
    }
    /** The SyntaxError for a regexp literal, worded as V8 words it. */
    fn regex_error(&self, pos: Pos, pattern: &str, flags: &str, reason: &str) -> EngineError {
        self.error_at(
            pos,
            &format!("Invalid regular expression: /{pattern}/{flags}: {reason}"),
        )
    }
    /** Literals, names, groups, functions, classes, arrays, objects and templates. */
    fn primary(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        let token = self.peek().clone();
        self.strict_literal(&token)?;
        let kind = match token.kind {
            #[cfg(feature = "conformance")]
            Kind::Operator(crate::v9::operator::Operator::Remainder) if self.native_fixture => {
                self.next();
                let name = self.next();
                let Kind::Name(name) = name.kind else {
                    return Err(self.unexpected());
                };
                let (intrinsic, count) = match name.as_str() {
                    "AbortJS" => (0, 1),
                    "PerformMicrotaskCheckpoint" => (1, 0),
                    "ArrayBufferDetach" => (2, 1),
                    "Is64Bit" => (3, 0),
                    "ToLength" => (4, 1),
                    "AtomicsSynchronizationPrimitiveNumWaitersForTesting" => (5, 1),
                    "IsJSReceiver" => (6, 1),
                    "IsArray" => (7, 1),
                    "ToBoolean" => (8, 1),
                    "Typeof" => (9, 1),
                    "ToName" => (10, 1),
                    "CreateIterResultObject" => (11, 2),
                    "GetWeakCollectionSize" => (13, 1),
                    "AtomicsNumWaitersForTesting" => (14, 2),
                    "AtomicsNumUnresolvedAsyncPromisesForTesting" => (15, 2),
                    "SetAllowAtomicsWait" => (16, 1),
                    "IsAtomicsWaitAllowed" => (17, 0),
                    "IsSharedString" => (18, 1),
                    "IsSameHeapObject" => (19, 2),
                    "SharedGC" => (20, 0),
                    "GetInitializerFunction" => (21, 1),
                    "ArrayBufferDetachForceWasm" => (22, 1),
                    "GetWasmExceptionTagId" => (23, 2),
                    "GetWasmExceptionValues" => (24, 1),
                    "ThrowStackOverflow" => (25, 0),
                    "StringMaxLength" => (26, 0),
                    "ShareObject" => (27, 1),
                    "IsInWritableSharedSpace" => (28, 1),
                    "ConstructConsString" => (31, 2),
                    "ConstructSlicedString" => (32, 2),
                    "FlattenString" => (35, 1),
                    "InYoungGeneration" => (36, 1),
                    "InternalizeString" => (37, 1),
                    "ConstructInternalizedString" => (38, 1),
                    "ConstructThinString" => (39, 1),
                    "AllocateHeapNumber" => (42, 0),
                    "IsSmi" => (43, 1),
                    "AllocateHeapNumberWithValue" => (44, 1),
                    "MaxSmi" => (46, 0),
                    "ArrayBufferMaxByteLength" => (47, 0),
                    "WasmStruct" => (48, 0),
                    "WasmArray" => (49, 0),
                    "IsWasmTrapHandlerEnabled" => (50, 0),
                    "StringLessThan" => (51, 2),
                    "StringLessThanOrEqual" => (52, 2),
                    "StringGreaterThan" => (53, 2),
                    "StringGreaterThanOrEqual" => (54, 2),
                    "StringEqual" => (55, 2),
                    "StringCompare" => (56, 2),
                    "Call" => (57, 2),
                    "ToNumber" => (58, 1),
                    "ToString" => (59, 1),
                    "ToNumeric" => (60, 1),
                    "ConstructDouble" => (61, 2),
                    "IsInternalizedString" => (62, 1),
                    "GetHoleNaN" => (63, 0),
                    "GetHoleNaNUpper" => (64, 0),
                    "GetHoleNaNLower" => (65, 0),
                    "HeapObjectVerify" => (66, 1),
                    "GetOptimizationStatus" => (68, 1),
                    "DebugPrint" | "CompileBaseline" => (71, 1),
                    "IsBeingInterpreted" | "ActiveTierIsIgnition" => (70, 0),
                    "IsMaglevEnabled"
                    | "IsTurbofanEnabled"
                    | "IsSparkplugEnabled"
                    | "IsConcurrentRecompilationSupported"
                    | "ActiveTierIsMaglev"
                    | "ActiveTierIsTurbofan"
                    | "ActiveTierIsSparkplug"
                    | "IsDictPropertyConstTrackingEnabled"
                    | "IsUndefinedDoubleEnabled"
                    | "PretenureAllocationSite"
                    | "CurrentFrameIsTurbofan" => (69, 0),
                    name if OPTIMIZER_HINTS.contains(&name) => (67, 0),
                    _ => {
                        return Err(crate::protocol::EngineError::unsupported(format!(
                            "V8 native intrinsic %{name}"
                        )));
                    }
                };
                let args = self.arguments()?;
                let intrinsic = if intrinsic == 44 && args.len() != 1 {
                    45
                } else if intrinsic == 38 && args.len() != 1 {
                    41
                } else if intrinsic == 39 && args.len() != 1 {
                    40
                } else if intrinsic == 31 && args.len() != 2 {
                    33
                } else if intrinsic == 32 && args.len() != 2 {
                    34
                } else if intrinsic == 27 && args.is_empty() {
                    29
                } else if intrinsic == 27 && args.len() > 1 {
                    30
                } else if intrinsic == 2 && args.len() == 2 {
                    12
                } else {
                    intrinsic
                };
                if (intrinsic == 57 && args.len() < 2)
                    || (matches!(intrinsic, 68 | 71) && !(1..=2).contains(&args.len()))
                    || (args.len() != count
                        && !matches!(
                            intrinsic,
                            12 | 29 | 30 | 33 | 34 | 40 | 41 | 45 | 57 | 67..=71
                        ))
                    || args.iter().any(|arg| !matches!(arg, Element::Value(_)))
                {
                    return Err(self.error_at(pos, "Invalid V8 intrinsic arguments"));
                }
                ExprKind::NativeIntrinsic(intrinsic, args)
            }
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
            Kind::BigIntWide(digits) => {
                self.next();
                ExprKind::BigIntWide(Name::from(digits.as_str()))
            }
            Kind::Regex { pattern, flags } => {
                self.regex_literal_check(pos, &pattern, &flags)?;
                self.next();
                ExprKind::Regex(Name::from(pattern.as_str()), Name::from(flags.as_str()))
            }
            Kind::Template { head: true, .. } => {
                let (quasis, _, expressions) = self.template_parts(false)?;
                ExprKind::Template(quasis, expressions)
            }
            Kind::Symbol('(') => {
                self.next();
                let value = self.expression(false)?;
                self.expect(")")?;
                // A parenthesized name is still a reference, but never a binding.
                if matches!(
                    value.kind,
                    ExprKind::Array(_)
                        | ExprKind::Object(_)
                        | ExprKind::Assign(..)
                        | ExprKind::Ident(_)
                ) {
                    self.parenthesized_patterns.insert(value.pos.offset);
                }
                if matches!(value.kind, ExprKind::Logical(..)) {
                    self.parenthesized_logical.insert(value.pos.offset);
                }
                return Ok(value);
            }
            Kind::Symbol('[') => return self.array_literal(),
            Kind::Symbol('{') => return self.object_literal(),
            Kind::Spread => {
                self.next();
                self.assignment(false)?;
                return Err(self.error_at(pos, "Unexpected token '...'"));
            }
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
        tagged: bool,
    ) -> EngineResult<(Vec<Option<JsString>>, Vec<JsString>, Vec<Expr>)> {
        let mut quasis = Vec::new();
        let mut raws = Vec::new();
        let mut expressions = Vec::new();
        let Kind::Template {
            cooked,
            raw,
            tail,
            invalid,
            ..
        } = self.next().kind
        else {
            return Err(self.unexpected());
        };
        if let (Some(reason), false) = (invalid, tagged) {
            return Err(self.error_at(self.pos(), reason));
        }
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
                    invalid,
                } => {
                    if let (Some(reason), false) = (invalid, tagged) {
                        return Err(self.error_at(self.pos(), reason));
                    }
                    quasis.push(cooked);
                    raws.push(raw);
                    tail = last;
                }
                _ => {
                    self.position -= 1;
                    return Err(self.unterminated_substitution());
                }
            }
        }
        Ok((quasis, raws, expressions))
    }
    /** The error where a template substitution's expression ends: a `}` the scanner could not
    resume the template after reports the scan error (an unterminated template), as V8 does once
    it rescans; anything else is the missing brace. */
    fn unterminated_substitution(&mut self) -> EngineError {
        if self.at("}") && matches!(self.peek_at(1).kind, Kind::Illegal(_)) {
            self.next();
        } else if !matches!(self.peek().kind, Kind::Illegal(_)) {
            return self.error_at(self.pos(), "Missing } in template expression");
        }
        self.unexpected()
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
                self.note_rest_comma(pos);
            } else {
                elements.push(Element::Value(self.assignment(false)?));
            }
            if !self.at("]") {
                self.expect(",")?;
            }
        }
        self.node(ExprKind::Array(elements), pos)
    }
    /** A comma after a spread is fine in a literal but makes it unusable as a pattern: an
    AssignmentRestElement must be the last thing in the list, with nothing (not even an elision)
    after it. The literal is parsed before anyone knows which it is, so the fact is kept by position. */
    fn note_rest_comma(&mut self, literal: Pos) {
        if self.at(",") {
            self.rest_commas.insert(literal.offset);
        }
    }
    /** `{a, b: c, [k]: v, m() {}, get x() {}, ...rest}` */
    fn object_literal(&mut self) -> EngineResult<Expr> {
        let pos = self.pos();
        self.expect("{")?;
        let mut properties = Vec::new();
        while !self.eat("}") {
            let property = self.property_definition()?;
            if matches!(property, PropDef::Spread(_)) {
                self.note_rest_comma(pos);
            }
            properties.push(property);
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
        let key_start = self.position;
        let key = self.property_key()?;
        if let PropKey::Private(name) = &key {
            return Err(self.error_at(pos, &format!("Unexpected identifier '#{name}'")));
        }
        // A key that cannot be shorthand must open a value or a method; V8 refuses the key itself.
        let bare = !matches!(self.tokens[key_start].kind, Kind::Name(_))
            && accessor.is_none()
            && !is_async
            && !generator;
        if bare && !self.at(":") && !self.at("(") {
            self.position = key_start;
            return Err(self.unexpected());
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
        if let PropKey::Name(name) = &key
            && &**name != "__proto__"
        {
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
        if !matches!(
            self.peek().kind,
            Kind::Name(_)
                | Kind::String(_)
                | Kind::Number(_)
                | Kind::BigInt(_)
                | Kind::BigIntWide(_)
                | Kind::Private(_)
                | Kind::Symbol('[')
        ) {
            return Err(self.unexpected());
        }
        let token = self.next();
        self.strict_literal(&token)?;
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
            Kind::BigInt(value) => PropKey::Name(Name::from(value.to_string())),
            Kind::BigIntWide(digits) => {
                PropKey::Name(Name::from(crate::v9::value::bigint_property_key(&digits)?))
            }
            Kind::Private(name) => PropKey::Private(Name::from(name.as_str())),
            Kind::Symbol('[') => {
                let key = self.assignment(false)?;
                self.expect("]")?;
                PropKey::Computed(Box::new(key))
            }
            _ => unreachable!("property keys are checked before they are consumed"),
        })
    }
    /** The cover grammar under one reading. The whole target refuses with the reading's own
    reason; every element inside it is a destructuring target, and a binding never holds a
    property reference. */
    pub(super) fn pattern_as(&mut self, value: Expr, cover: Cover) -> EngineResult<Pattern> {
        let pos = value.pos;
        self.check_cover(&value, cover)?;
        match value.kind {
            ExprKind::Ident(name) => {
                self.check_assignable(&name, pos, cover)?;
                Ok(Pattern::Ident(name, pos))
            }
            ExprKind::Member(..) if !cover.binding() => Ok(Pattern::Expr(Box::new(value))),
            ExprKind::Member(..) if cover == Cover::BindingElement => {
                Err(self.error_at(pos, ILLEGAL_BINDING_PROPERTY))
            }
            ExprKind::Array(elements) => self.array_pattern(elements, pos, cover),
            ExprKind::Object(properties) => self.object_pattern(properties, pos, cover),
            ExprKind::Assign(AssignOp::Assign, target, _)
                if cover != Cover::Assignment && legacy_call(&target) =>
            {
                Err(self.error_at(pos, INVALID_TARGET))
            }
            ExprKind::Assign(AssignOp::Assign, target, value) => {
                Ok(Pattern::Default(target, value))
            }
            _ => Err(self.error_at(pos, cover.reason())),
        }
    }
    /** `[a, , ...rest]` as a pattern: a rest with a default is no target, and nothing follows a rest. */
    fn array_pattern(
        &mut self,
        elements: Vec<Element>,
        pos: Pos,
        cover: Cover,
    ) -> EngineResult<Pattern> {
        let mut items = Vec::new();
        let mut rest = None;
        let count = elements.len();
        for (index, element) in elements.into_iter().enumerate() {
            match element {
                Element::Hole => items.push(None),
                Element::Value(value) => {
                    items.push(Some(self.pattern_as(value, cover.element())?));
                }
                Element::Spread(value) if matches!(value.kind, ExprKind::Assign(..)) => {
                    return Err(self.error_at(value.pos, INVALID_TARGET));
                }
                Element::Spread(value) if index + 1 == count => {
                    rest = Some(Box::new(self.pattern_as(value, cover.element())?));
                }
                Element::Spread(_) => return Err(self.error_at(pos, REST_NOT_LAST)),
            }
        }
        Ok(Pattern::Array(items, rest, pos))
    }
    /** `{a, b: c, ...rest}` as a pattern: methods and accessors are no targets, the rest must
    be a plain reference (a name, for a binding), and nothing follows it. */
    fn object_pattern(
        &mut self,
        properties: Vec<PropDef>,
        pos: Pos,
        cover: Cover,
    ) -> EngineResult<Pattern> {
        let mut items = Vec::new();
        let mut rest = None;
        let count = properties.len();
        for (index, property) in properties.into_iter().enumerate() {
            match property {
                PropDef::Value(key, value) => items.push(PatProp {
                    key,
                    value: self.pattern_as(value, cover.element())?,
                }),
                PropDef::Shorthand(name, pos) => {
                    self.check_assignable(&name, pos, Cover::Assignment)?;
                    items.push(PatProp {
                        key: PropKey::Name(name.clone()),
                        value: Pattern::Ident(name, pos),
                    })
                }
                PropDef::ShorthandDefault(name, pos, value) => {
                    self.check_assignable(&name, pos, Cover::Assignment)?;
                    items.push(PatProp {
                        key: PropKey::Name(name.clone()),
                        value: Pattern::Default(Box::new(Pattern::Ident(name, pos)), value),
                    })
                }
                PropDef::Spread(value) if index + 1 == count => {
                    rest = Some(Box::new(self.object_rest(value, cover)?));
                }
                PropDef::Spread(_) => return Err(self.error_at(pos, REST_NOT_LAST)),
                _ => return Err(self.error_at(pos, INVALID_TARGET)),
            }
        }
        Ok(Pattern::Object(items, rest, pos))
    }
    /** An object rest target: a reference in assignments, a name in bindings; a nested pattern
    or default is V8's "must be followed by" error, anything else is no target at all. */
    fn object_rest(&mut self, value: Expr, cover: Cover) -> EngineResult<Pattern> {
        let pattern = matches!(
            value.kind,
            ExprKind::Array(_) | ExprKind::Object(_) | ExprKind::Assign(..)
        );
        let reference = matches!(value.kind, ExprKind::Ident(_) | ExprKind::Member(..));
        if pattern {
            return Err(self.error_at(value.pos, cover.rest_reason()));
        }
        if !reference {
            return Err(self.error_at(value.pos, cover.element().reason()));
        }
        self.pattern_as(value, cover.element())
    }
    /** What the cover grammar refuses to reread as a pattern: a parenthesized literal or
    assignment (`(x = y) = 1`, `[(x = y)] = []`), and a literal with a comma after its rest. */
    fn check_cover(&self, value: &Expr, cover: Cover) -> EngineResult<()> {
        let pos = value.pos;
        let literal = matches!(value.kind, ExprKind::Array(_) | ExprKind::Object(_));
        if self.parenthesized_patterns.contains(&pos.offset)
            && (literal || matches!(value.kind, ExprKind::Assign(..)) || cover.binding())
        {
            return Err(self.error_at(pos, cover.reason()));
        }
        if literal && self.rest_commas.contains(&pos.offset) {
            return Err(self.error_at(pos, REST_NOT_LAST));
        }
        Ok(())
    }
    /** Web-compatible call targets are rewritten to call[throw ReferenceError], as in V8; the
    web-compat allowance is sloppy only, so strict code gets the early error instead. */
    pub(super) fn legacy_target(&mut self, value: Expr) -> EngineResult<Pattern> {
        self.legacy_target_as(value, Cover::Assignment)
    }
    /** `legacy_target` under a reading: the for-in/of head words its refusal differently. */
    pub(super) fn legacy_target_as(&mut self, value: Expr, cover: Cover) -> EngineResult<Pattern> {
        if matches!(value.kind, ExprKind::Call(_, _, false)) {
            let pos = value.pos;
            if self.strict {
                return Err(self.error_at(pos, cover.reason()));
            }
            let error = self.node(ExprKind::InvalidReference(INVALID_ASSIGNMENT), pos)?;
            let member = self.node(
                ExprKind::Member(
                    Box::new(value),
                    MemberProp::Computed(Box::new(error)),
                    false,
                ),
                pos,
            )?;
            return Ok(Pattern::Expr(Box::new(member)));
        }
        self.pattern_as(value, cover)
    }
    /** Compound assignment and updates need a plain name or member target; `reason` is what
    the operation calls anything else. */
    fn simple_target(&mut self, value: Expr, reason: &'static str) -> EngineResult<Pattern> {
        if matches!(value.kind, ExprKind::Call(_, _, false)) {
            return self.legacy_target(value);
        }
        let pos = value.pos;
        match value.kind {
            ExprKind::Ident(name) => {
                self.check_assignable(&name, pos, Cover::Assignment)?;
                Ok(Pattern::Ident(name, pos))
            }
            ExprKind::Member(..) => Ok(Pattern::Expr(Box::new(value))),
            _ => Err(self.error_at(pos, reason)),
        }
    }
    /** Strict code cannot assign `eval` or `arguments`; V8 says so for a plain target and
    calls the name an invalid element inside a pattern. */
    fn check_assignable(&self, name: &str, pos: Pos, cover: Cover) -> EngineResult<()> {
        if self.strict && (name == "eval" || name == "arguments") {
            let reason = match cover {
                Cover::Assignment | Cover::ForEach => "Unexpected eval or arguments in strict mode",
                _ => INVALID_TARGET,
            };
            return Err(self.error_at(pos, reason));
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
        let simple = matches!(&self.peek_at(offset).kind, Kind::Name(name) if self.arrow_name(name))
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
    /** True if a name may head a simple arrow: a reserved word is not one, but `yield` and
    `await` are except where they are operators, so the arrow reports the strict reserved word
    and the operator reports the `=>`. */
    fn arrow_name(&self, name: &str) -> bool {
        match name {
            "yield" => !self.context.generator,
            "await" => !self.context.asynchronous,
            _ => !reserved(name, self.strict),
        }
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
/** The precedence of `??`; its operands are read one level above it. */
const NULLISH: u8 = 1;
/** The level a `??` operand is read at: a `||` or `&&` met there mixes with the `??`. */
const NULLISH_OPERAND: u8 = NULLISH + 1;
/** V8's reason for an assignment whose target is not a reference or pattern. */
pub(super) const INVALID_ASSIGNMENT: &str = "Invalid left-hand side in assignment";
/** V8's reason for a pattern element that cannot be assigned. */
pub(super) const INVALID_TARGET: &str = "Invalid destructuring assignment target";
/** V8's reason for a for-in/of head that is not a reference or pattern. */
pub(super) const INVALID_FOR_TARGET: &str = "Invalid left-hand side in for-loop";
/** V8's reason for a rest element with anything after it. */
const REST_NOT_LAST: &str = "Rest element must be last element";
/** V8's reason for a property reference where a binding pattern wants a name. */
const ILLEGAL_BINDING_PROPERTY: &str = "Illegal property in declaration context";
/** V8's reason for an object rest that is a pattern or a default in an assignment. */
const REST_ASSIGNABLE: &str =
    "`...` must be followed by an assignable reference in assignment contexts";
/** V8's reason for an object rest that is a pattern or a default in a binding. */
const REST_IDENTIFIER: &str = "`...` must be followed by an identifier in declaration contexts";

/** How a cover expression is reread: as the whole target of an assignment or a for-in/of head,
as an element inside a destructuring target, or as the bindings of an arrow head. Each reading
words its refusals as V8 does. */
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Cover {
    /** The target of `=`. */
    Assignment,
    /** The head of a for-in/of loop. */
    ForEach,
    /** An element of a destructuring assignment. */
    Element,
    /** One arrow parameter, reread from a parenthesized expression. */
    Binding,
    /** An element inside a reread arrow parameter. */
    BindingElement,
}

impl Cover {
    /** The reading of an element inside this one. */
    fn element(self) -> Self {
        match self {
            Cover::Binding | Cover::BindingElement => Cover::BindingElement,
            _ => Cover::Element,
        }
    }
    /** True for the readings that bind names rather than assign references. */
    fn binding(self) -> bool {
        matches!(self, Cover::Binding | Cover::BindingElement)
    }
    /** What this reading calls an expression it cannot take. */
    fn reason(self) -> &'static str {
        match self {
            Cover::Assignment => INVALID_ASSIGNMENT,
            Cover::ForEach => INVALID_FOR_TARGET,
            Cover::Element | Cover::Binding | Cover::BindingElement => INVALID_TARGET,
        }
    }
    /** What this reading calls an object rest that is a pattern or a default. */
    fn rest_reason(self) -> &'static str {
        if self.binding() {
            REST_IDENTIFIER
        } else {
            REST_ASSIGNABLE
        }
    }
}

/** True for the target `legacy_target` builds for a sloppy call: a reference that throws when
assigned, which no destructuring element may be. */
fn legacy_call(target: &Pattern) -> bool {
    match target {
        Pattern::Expr(expr) => matches!(
            &expr.kind,
            ExprKind::Member(base, MemberProp::Computed(key), _)
                if matches!(base.kind, ExprKind::Call(..))
                    && matches!(key.kind, ExprKind::InvalidReference(_))
        ),
        _ => false,
    }
}
/** V8's reason for a prefix `++` or `--` on a non-reference. */
const INVALID_PREFIX: &str = "Invalid left-hand side expression in prefix operation";
/** V8's reason for a postfix `++` or `--` on a non-reference. */
const INVALID_POSTFIX: &str = "Invalid left-hand side expression in postfix operation";
/** V8's reason for a unary operator applied to an exponentiation base. */
const UNARY_EXPONENT: &str = "Unary operator used immediately before exponentiation expression. \
Parenthesis must be used to disambiguate operator precedence";
/** V8's reason for an `import` that is not `import(` or `import.` in a classic script. */
pub(super) const IMPORT_OUTSIDE_MODULE: &str = "Cannot use import statement outside a module";
/** V8's reason for a call whose arguments are not followed by `,` or `)`. */
const UNTERMINATED_ARGUMENTS: &str = "missing ) after argument list";
/** V8's reason for an `await` expression in async parameters. */
const AWAIT_IN_PARAMETERS: &str = "Illegal await-expression in formal parameters of async function";

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
