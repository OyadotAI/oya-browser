/** Expression lowering: operators, member access, calls, literals and optional chains. */
use super::function::{GETTER, METHOD, SETTER};
use super::*;
use crate::v9::syntax::{
    AssignOp, BinaryOp, Element, ExprKind, LogicalOp, MemberProp, Pattern, PropDef, PropKey,
    UnaryOp,
};

impl Compiler {
    /** Push the value of an expression. */
    pub(super) fn expr(&mut self, expr: &Expr) -> EngineResult<()> {
        self.at(expr.pos);
        match &expr.kind {
            ExprKind::Number(value) => {
                self.emit(Op::Number(*value));
            }
            ExprKind::String(value) => {
                let index = self.string(value);
                self.emit(Op::String(index));
            }
            ExprKind::Opaque(feature) => {
                let index = self.name(feature)?;
                self.emit(Op::Unsupported(index));
            }
            #[cfg(feature = "conformance")]
            ExprKind::NativeIntrinsic(intrinsic, args) if (67..=71).contains(intrinsic) => {
                self.optimizer_intrinsic(*intrinsic, args)?;
            }
            #[cfg(feature = "conformance")]
            ExprKind::NativeIntrinsic(intrinsic, args) => {
                if *intrinsic == 29 {
                    self.emit(Op::Undefined);
                }
                for (index, arg) in args.iter().enumerate() {
                    if let Element::Value(value) = arg {
                        self.expr(value)?;
                        if (*intrinsic == 30 && index > 0)
                            || (matches!(*intrinsic, 31..=34) && index > 1)
                            || (matches!(*intrinsic, 40 | 41 | 45) && index > 0)
                        {
                            self.emit(Op::Pop);
                        }
                    }
                }
                if matches!(*intrinsic, 40 | 41 | 45) && args.is_empty() {
                    self.emit(Op::Undefined);
                }
                if matches!(*intrinsic, 31..=34) {
                    for _ in args.len().min(2)..2 {
                        self.emit(Op::Undefined);
                    }
                }
                if *intrinsic == 57 {
                    self.emit(Op::Call(count(&args[2..])?));
                } else {
                    self.emit(Op::NativeIntrinsic(*intrinsic));
                }
            }
            ExprKind::InvalidReference(message) => {
                let index = self.name(message)?;
                self.emit(Op::ThrowReference(index));
            }
            ExprKind::BigInt(value) => {
                let index = self.name(&value.to_string())?;
                self.emit(Op::BigInt(index));
            }
            ExprKind::Template(quasis, values) => self.template(quasis, values)?,
            ExprKind::BigIntWide(digits) => {
                let index = self.name(digits)?;
                self.emit(Op::BigInt(index));
            }
            ExprKind::Tagged(tag, quasis, raws, values) => {
                self.tagged(tag, quasis, raws, values)?
            }
            ExprKind::Regex(pattern, flags) => {
                let pattern = self.name(pattern)?;
                let flags = self.name(flags)?;
                self.emit(Op::Regex(pattern, flags));
            }
            ExprKind::Ident(name) => self.load_name(name, expr.pos)?,
            ExprKind::This => self.load_this()?,
            ExprKind::Super => {
                let index = self.name("super")?;
                self.emit(Op::Unsupported(index));
            }
            ExprKind::NewTarget => {
                if self.resolve("new.target").is_some() {
                    self.load_name(&Name::from("new.target"), expr.pos)?;
                } else {
                    self.emit(Op::Undefined);
                }
            }
            ExprKind::Null => {
                self.emit(Op::Null);
            }
            ExprKind::Bool(value) => {
                self.emit(if *value { Op::True } else { Op::False });
            }
            ExprKind::Array(elements) => self.array(elements)?,
            ExprKind::Object(props) => self.object(props)?,
            ExprKind::Function(func) => self.closure(func, !func.flags.arrow)?,
            ExprKind::Class(class) => self.class(class)?,
            ExprKind::Unary(operator, operand) => self.unary(*operator, operand)?,
            ExprKind::Update(increment, prefix, target) => {
                self.update(*increment, *prefix, target)?
            }
            ExprKind::Binary(operator, left, right) => {
                self.expr(left)?;
                self.expr(right)?;
                self.at(expr.pos);
                self.emit(binary(*operator));
            }
            ExprKind::Logical(operator, left, right) => {
                self.expr(left)?;
                let jump = self.emit(logical(*operator));
                self.expr(right)?;
                self.patch(jump);
            }
            ExprKind::Assign(operator, target, value) => self.assign(*operator, target, value)?,
            ExprKind::Conditional(test, yes, no) => {
                self.expr(test)?;
                let other = self.emit(Op::JumpIfFalse(0));
                self.expr(yes)?;
                let end = self.emit(Op::Jump(0));
                self.patch(other);
                self.expr(no)?;
                self.patch(end);
            }
            ExprKind::Call(callee, args, optional) => {
                self.call(callee, args, *optional, expr.pos)?
            }
            ExprKind::New(callee, args) => {
                self.expr(callee)?;
                if spreads(args) {
                    self.spread_array(args, true)?;
                    self.at(expr.pos);
                    self.name_callee(callee);
                    self.emit(Op::NewSpread);
                } else {
                    for arg in args {
                        self.element_value(arg)?;
                    }
                    self.at(expr.pos);
                    self.name_callee(callee);
                    self.emit(Op::New(count(args)?));
                }
            }
            ExprKind::Member(object, prop, optional) => {
                // A read of `super.x` goes through the home object; `this.x` would find the
                // method's own override (Polymer's mixins test `super.connectedCallback`).
                let is_super = matches!(object.kind, ExprKind::Super);
                self.member_object(object, *optional)?;
                self.at(expr.pos);
                self.member_get_from(prop, is_super)?;
            }
            ExprKind::Chain(inner) => {
                self.f().chains.push(Chain::default());
                self.expr(inner)?;
                let chain = self.f().chains.pop().expect("chain");
                if chain.tramps.is_empty() {
                    for exit in chain.exits {
                        self.patch(exit);
                    }
                } else {
                    let end = self.emit(Op::Jump(0));
                    for tramp in chain.tramps {
                        self.patch(tramp);
                    }
                    self.emit(Op::Swap);
                    self.emit(Op::Pop);
                    for exit in chain.exits {
                        self.patch(exit);
                    }
                    self.patch(end);
                }
            }
            ExprKind::Sequence(values) => {
                for (index, value) in values.iter().enumerate() {
                    self.expr(value)?;
                    if index + 1 < values.len() {
                        self.emit(Op::Pop);
                    }
                }
            }
            ExprKind::Yield(value, delegate) => {
                match value {
                    Some(value) => self.expr(value)?,
                    None => {
                        self.emit(Op::Undefined);
                    }
                }
                self.at(expr.pos);
                // An async generator awaits what it yields, and delegates to an async iterator.
                let asynchronous = self.f().flags.asynchronous;
                if *delegate && asynchronous {
                    self.emit(Op::GetAsyncIterator);
                    self.emit(Op::Undefined);
                    self.emit(Op::AsyncYieldStar);
                    self.emit(Op::AsyncYieldStarResult);
                } else if *delegate {
                    if value
                        .as_ref()
                        .is_some_and(|value| !site_text::is_name_or_literal(value))
                    {
                        self.record_site(SiteKind::Printed, YIELD_STAR_VALUE);
                    }
                    self.emit(Op::GetIterator);
                    self.emit(Op::Undefined);
                    self.emit(Op::YieldStar);
                } else {
                    if asynchronous {
                        self.emit(Op::Await);
                    }
                    self.emit(Op::Yield);
                }
            }
            ExprKind::Await(value) => {
                self.expr(value)?;
                self.at(expr.pos);
                self.emit(Op::Await);
            }
            ExprKind::ImportMeta => self.load_name(&Name::from(super::module::META), expr.pos)?,
            ExprKind::Import(specifier, options) => {
                self.expr(specifier)?;
                match options {
                    Some(options) => self.expr(options)?,
                    None => {
                        self.emit(Op::Undefined);
                    }
                }
                self.at(expr.pos);
                self.emit(Op::Import);
            }
            ExprKind::PrivateIn(name, object) => {
                let key = self.private_name(name)?;
                self.expr(object)?;
                self.emit(Op::PrivateIn(key));
            }
        }
        Ok(())
    }
    /** Evaluate a member's object, short-circuiting an optional link. */
    fn member_object(&mut self, object: &Expr, optional: bool) -> EngineResult<()> {
        if matches!(object.kind, ExprKind::Super) {
            self.load_this()?;
            return Ok(());
        }
        self.expr(object)?;
        if optional {
            let exit = self.emit(Op::JumpIfNullish(0));
            if let Some(chain) = self.f().chains.last_mut() {
                chain.exits.push(exit);
            }
        }
        Ok(())
    }
    /** object -> value; `super` members were given `this` as their object. */
    fn member_get(&mut self, prop: &MemberProp) -> EngineResult<()> {
        self.member_get_from(prop, false)
    }
    /** object -> value, through the home object for `super`. */
    fn member_get_from(&mut self, prop: &MemberProp, is_super: bool) -> EngineResult<()> {
        match prop {
            MemberProp::Name(name) => {
                let index = self.name(name)?;
                if is_super {
                    self.load_home()?;
                    self.emit(Op::GetSuper(index));
                } else {
                    self.emit(Op::GetProp(index));
                }
            }
            MemberProp::Private(name) => {
                let index = self.private_name(name)?;
                self.emit(Op::GetProp(index));
            }
            MemberProp::Computed(key) => {
                self.expr(key)?;
                if is_super {
                    self.load_home()?;
                    self.emit(Op::GetSuperElem);
                } else {
                    self.emit(Op::GetElem);
                }
            }
        }
        Ok(())
    }
    /** Parentheses end short-circuit propagation while retaining a successful member's reference receiver. */
    fn chain_reference(&mut self, inner: &Expr) -> EngineResult<()> {
        let ExprKind::Member(object, prop, optional) = &inner.kind else {
            unreachable!("member reference")
        };
        self.f().chains.push(Chain::default());
        self.member_object(object, *optional)?;
        self.emit(Op::Dup);
        self.member_get_from(prop, matches!(object.kind, ExprKind::Super))?;
        let chain = self.f().chains.pop().expect("reference chain");
        let end = self.emit(Op::Jump(0));
        if !chain.tramps.is_empty() {
            for tramp in chain.tramps {
                self.patch(tramp);
            }
            self.emit(Op::Swap);
            self.emit(Op::Pop);
        }
        for exit in chain.exits {
            self.patch(exit);
        }
        self.emit(Op::Undefined);
        self.patch(end);
        Ok(())
    }
    /** Calls: method calls pass their object as `this`; optional calls short-circuit. */
    fn call(
        &mut self,
        callee: &Expr,
        args: &[Element],
        optional: bool,
        pos: Pos,
    ) -> EngineResult<()> {
        let mut method = false;
        // The callee is a name that pushed its own `this` (a `with` object or undefined).
        let mut receiver = false;
        match &callee.kind {
            ExprKind::Member(object, prop, member_optional) => {
                let is_super = matches!(object.kind, ExprKind::Super);
                self.member_object(object, *member_optional)?;
                self.emit(Op::Dup);
                self.at(callee.pos);
                self.member_get_from(prop, is_super)?;
                method = true;
            }
            ExprKind::Chain(inner) if matches!(inner.kind, ExprKind::Member(..)) => {
                self.chain_reference(inner)?;
                method = true;
            }
            ExprKind::Super => {
                return self.super_call(args, pos);
            }
            ExprKind::Ident(name) if !optional => {
                self.callee_name(name, callee.pos)?;
                receiver = true;
            }
            _ => {
                self.expr(callee)?;
            }
        }
        if optional {
            if method {
                let tramp = self.emit(Op::JumpIfNullish(0));
                if let Some(chain) = self.f().chains.last_mut() {
                    chain.tramps.push(tramp);
                }
            } else {
                let exit = self.emit(Op::JumpIfNullish(0));
                if let Some(chain) = self.f().chains.last_mut() {
                    chain.exits.push(exit);
                }
            }
        }
        if method {
            self.emit(Op::Swap);
        } else if !receiver {
            self.emit(Op::Undefined);
        }
        let direct = !optional
            && !spreads(args)
            && matches!(&callee.kind, ExprKind::Ident(name) if &**name == "eval");
        if direct {
            for arg in args {
                self.element_value(arg)?;
            }
            self.at(pos);
            return self.direct_eval_call(count(args)?);
        }
        if spreads(args) {
            self.spread_array(args, true)?;
            self.at(pos);
            self.name_callee(callee);
            self.emit(Op::CallSpread);
        } else {
            for arg in args {
                self.element_value(arg)?;
            }
            self.at(pos);
            self.name_callee(callee);
            self.emit(Op::Call(count(args)?));
        }
        Ok(())
    }
    /** Remember the callee as the source wrote it for the call instruction emitted next, so a
     * call of a non-function names it as V8 does. */
    fn name_callee(&mut self, callee: &Expr) {
        self.record_site(SiteKind::Printed, &site_text::site_text(callee));
    }
    /** Record the written form of the operand of the instruction emitted next, for its message. */
    pub(super) fn record_site(&mut self, kind: SiteKind, text: &str) {
        let pc = self.here() as u32;
        self.f().callees.push((pc, kind, text.into()));
    }
    /** Record how a value an instruction iterates was written: `for (x of y.z)`, `[...y.z]`.
     * A call prints as its callee, since V8 blames the call before the iteration. */
    pub(super) fn record_iterable(&mut self, iterable: &Expr) {
        match site_text::call_callee(iterable) {
            Some(callee) => self.record_site(SiteKind::CallResult, &site_text::site_text(callee)),
            None => self.record_site(SiteKind::Printed, &site_text::site_text(iterable)),
        }
    }
    /** Record an array spread while retaining its distinct nullish error wording. */
    fn record_array_spread(&mut self, iterable: &Expr) {
        match site_text::call_callee(iterable) {
            Some(callee) => self.record_site(SiteKind::CallResult, &site_text::site_text(callee)),
            None => self.record_site(SiteKind::ArraySpread, &site_text::site_text(iterable)),
        }
    }
    /** Record a value an array pattern iterates. V8 prints it only when its printer reaches it;
     * anything else is described by its runtime type. */
    pub(super) fn record_destructured_iterable(&mut self, iterable: &Expr) {
        if site_text::starts_at_first_token(iterable) {
            self.record_iterable(iterable);
        }
    }
    /** `super(...)` binds `this`. */
    fn super_call(&mut self, args: &[Element], pos: Pos) -> EngineResult<()> {
        if self.resolve("%constructor").is_some()
            && matches!(self.resolve("this"), Some((Place::Env(_), _, _)))
        {
            return self.lexical_super_call(args, pos);
        }
        if spreads(args) {
            self.spread_array(args, true)?;
            self.at(pos);
            self.emit(Op::SuperCallSpread);
        } else {
            for arg in args {
                self.element_value(arg)?;
            }
            self.at(pos);
            self.emit(Op::SuperCall(count(args)?));
        }
        if self.resolve("this").is_some() {
            self.initialize(&Name::from("this"), pos)?;
        }
        self.emit(Op::InitializeDerivedFields);
        Ok(())
    }
    /** Retain the enclosing constructor across suspension and bind its captured receiver after construction. */
    fn lexical_super_call(&mut self, args: &[Element], pos: Pos) -> EngineResult<()> {
        self.load_name(&Name::from("%constructor"), pos)?;
        self.load_name(&Name::from("new.target"), pos)?;
        if spreads(args) {
            self.spread_array(args, true)?;
            self.emit(Op::LexicalSuperCallSpread);
        } else {
            for arg in args {
                self.element_value(arg)?;
            }
            self.emit(Op::LexicalSuperCall(count(args)?));
        }
        self.load_name(&Name::from("%constructor"), pos)?;
        match self.resolve("this") {
            Some((Place::Env(slot), _, hops)) => self.emit(Op::BindLexicalSuper(hops, slot)),
            _ => return Err(self.error(pos, "Missing lexical constructor environment")),
        };
        Ok(())
    }
    /** A plain list element's value. */
    fn element_value(&mut self, element: &Element) -> EngineResult<()> {
        match element {
            Element::Value(value) => self.expr(value),
            Element::Spread(value) => self.expr(value),
            Element::Hole => {
                self.emit(Op::Hole);
                Ok(())
            }
        }
    }
    /** Build an array from elements with spreads. */
    fn spread_array(&mut self, elements: &[Element], arguments: bool) -> EngineResult<()> {
        self.emit(Op::NewArray(0));
        for element in elements {
            match element {
                Element::Value(value) => {
                    self.expr(value)?;
                    self.emit(Op::ArrayPush);
                }
                Element::Spread(value) => {
                    self.expr(value)?;
                    if arguments {
                        self.record_site(SiteKind::SpreadArgument, &site_text::site_text(value));
                    } else {
                        self.record_array_spread(value);
                    }
                    self.emit(Op::ArraySpread);
                }
                Element::Hole => {
                    self.emit(Op::Hole);
                    self.emit(Op::ArrayPush);
                }
            }
        }
        Ok(())
    }
    /** Array literals. */
    fn array(&mut self, elements: &[Element]) -> EngineResult<()> {
        if spreads(elements) || elements.len() > ARRAY_LITERAL_BATCH {
            return self.spread_array(elements, false);
        }
        for element in elements {
            self.element_value(element)?;
        }
        self.emit(Op::NewArray(elements.len() as u32));
        Ok(())
    }
    /** Object literals. */
    fn object(&mut self, props: &[PropDef]) -> EngineResult<()> {
        literal_prototypes(props)?;
        self.emit(Op::NewObject);
        for prop in props {
            match prop {
                PropDef::Value(PropKey::Name(name), value) if &**name == "__proto__" => {
                    self.expr(value)?;
                    self.emit(Op::SetProtoLiteral);
                }
                PropDef::Value(PropKey::Computed(key), value) => {
                    let mark = self.f().registers;
                    let name = self.temp()?;
                    self.expr(key)?;
                    self.emit(Op::ToPropertyKey);
                    self.emit(Op::SetReg(name));
                    self.field_value_named(Some(value), name)?;
                    self.emit(Op::DefineFieldComputed);
                    self.release(mark);
                }
                PropDef::Value(key, value) => {
                    let mark = self.f().registers;
                    let name = self.temp()?;
                    let index = self.key_index(key)?;
                    self.emit(Op::String(index));
                    self.emit(Op::SetReg(name));
                    self.emit(Op::Pop);
                    self.field_value_named(Some(value), name)?;
                    self.emit(Op::DefineField(index));
                    self.release(mark);
                }
                PropDef::Shorthand(name, pos) => {
                    let index = self.name(name)?;
                    self.load_name(name, *pos)?;
                    self.emit(Op::DefineField(index));
                }
                PropDef::ShorthandDefault(_, pos, _) => {
                    return Err(self.error(*pos, "Invalid shorthand property initializer"));
                }
                PropDef::Method(key, func) => self.literal_method(key, func, METHOD)?,
                PropDef::Getter(key, func) => self.literal_method(key, func, GETTER)?,
                PropDef::Setter(key, func) => self.literal_method(key, func, SETTER)?,
                PropDef::Spread(value) => {
                    self.expr(value)?;
                    self.emit(Op::CopyData);
                }
            }
        }
        Ok(())
    }
    /** An enumerable object-literal method or accessor. */
    fn literal_method(&mut self, key: &PropKey, func: &Arc<Func>, kind: u8) -> EngineResult<()> {
        match key {
            PropKey::Computed(key) => {
                self.expr(key)?;
                self.emit(Op::ToPropertyKey);
                self.closure(func, false)?;
                self.emit(Op::DefineMethodComputed(kind, true));
            }
            key => {
                let index = self.key_index(key)?;
                self.closure(func, false)?;
                self.emit(Op::DefineMethod(index, kind, true));
            }
        }
        Ok(())
    }
    /** Untagged templates concatenate strings. */
    fn template(&mut self, quasis: &[Option<JsString>], values: &[Expr]) -> EngineResult<()> {
        if quasis.iter().any(Option::is_none) {
            let index = self.name("template literals with invalid escapes")?;
            self.emit(Op::Unsupported(index));
            return Ok(());
        }
        let first = quasis[0].clone().expect("cooked");
        let index = self.string(&first);
        self.emit(Op::String(index));
        for (value, chunk) in values.iter().zip(&quasis[1..]) {
            self.expr(value)?;
            self.emit(Op::ToStr);
            self.emit(Op::Add);
            let chunk = chunk.clone().expect("cooked");
            if !chunk.is_empty() {
                let index = self.string(&chunk);
                self.emit(Op::String(index));
                self.emit(Op::Add);
            }
        }
        Ok(())
    }
    /** Tagged templates call the tag with a strings array and the substitutions. */
    fn tagged(
        &mut self,
        tag: &Expr,
        quasis: &[Option<JsString>],
        raws: &[JsString],
        values: &[Expr],
    ) -> EngineResult<()> {
        match &tag.kind {
            ExprKind::Chain(inner) if matches!(inner.kind, ExprKind::Member(..)) => {
                self.chain_reference(inner)?;
                self.emit(Op::Swap);
            }
            ExprKind::Member(object, prop, _) => {
                self.expr(object)?;
                self.emit(Op::Dup);
                self.member_get(prop)?;
                self.emit(Op::Swap);
            }
            _ => {
                self.expr(tag)?;
                self.emit(Op::Undefined);
            }
        }
        let index = self.f().templates.len() as u32;
        self.f()
            .templates
            .push(quasis.iter().cloned().zip(raws.iter().cloned()).collect());
        self.emit(Op::TemplateObject(index));
        for value in values {
            self.expr(value)?;
        }
        let argc = Slot::try_from(values.len() + 1)
            .map_err(|_| EngineError::limit("V9 call arguments", Slot::MAX as usize))?;
        self.name_callee(tag);
        self.emit(Op::Call(argc));
        Ok(())
    }
    /** Prefix operators. */
    fn unary(&mut self, operator: UnaryOp, operand: &Expr) -> EngineResult<()> {
        match operator {
            UnaryOp::Typeof => {
                if let ExprKind::Ident(name) = &operand.kind
                    && matches!(self.resolve(name), None | Some((Place::Global, ..)))
                {
                    let ext = self.ext_loads(name)?;
                    let index = self.name(name)?;
                    self.emit(Op::TypeofGlobal(index));
                    if !ext.is_empty() {
                        let end = self.emit(Op::Jump(0));
                        self.patch_all(ext);
                        self.emit(Op::Typeof);
                        self.patch(end);
                    }
                } else {
                    self.expr(operand)?;
                    self.emit(Op::Typeof);
                }
            }
            UnaryOp::Delete => self.delete(operand)?,
            UnaryOp::Void => {
                self.expr(operand)?;
                self.emit(Op::Pop);
                self.emit(Op::Undefined);
            }
            UnaryOp::Minus => {
                if let ExprKind::Number(value) = operand.kind {
                    self.emit(Op::Number(-value));
                } else {
                    self.expr(operand)?;
                    self.emit(Op::Neg);
                }
            }
            UnaryOp::Plus => {
                self.expr(operand)?;
                self.emit(Op::ToNumber);
            }
            UnaryOp::Not => {
                self.expr(operand)?;
                self.emit(Op::Not);
            }
            UnaryOp::BitNot => {
                self.expr(operand)?;
                self.emit(Op::BitNot);
            }
        }
        Ok(())
    }
    /** `delete` of members, global names and other values. */
    fn delete(&mut self, operand: &Expr) -> EngineResult<()> {
        match &operand.kind {
            ExprKind::Member(object, prop, optional) => {
                if matches!(object.kind, ExprKind::Super) {
                    self.load_this()?;
                    match prop {
                        MemberProp::Computed(key) => {
                            self.expr(key)?;
                            self.emit(Op::Pop);
                        }
                        MemberProp::Name(_) => {}
                        MemberProp::Private(_) => {
                            return Err(
                                self.error(operand.pos, "Private fields can not be deleted")
                            );
                        }
                    }
                    self.load_home()?;
                    self.emit(Op::CaptureSuperBase);
                    self.emit(Op::Pop);
                    self.emit(Op::Pop);
                    let message = self.name("Unsupported reference to 'super'")?;
                    self.at(operand.pos);
                    self.emit(Op::ThrowReference(message));
                    return Ok(());
                }
                self.member_object(object, *optional)?;
                match prop {
                    MemberProp::Name(name) => {
                        let index = self.name(name)?;
                        self.emit(Op::DeleteProp(index));
                    }
                    MemberProp::Private(_) => {
                        // Early error (spec 13.5.1.1): `delete` of a private reference.
                        return Err(self.error(operand.pos, "Private fields can not be deleted"));
                    }
                    MemberProp::Computed(key) => {
                        self.expr(key)?;
                        self.emit(Op::DeleteElem);
                    }
                }
            }
            ExprKind::Chain(inner) => {
                self.f().chains.push(Chain::default());
                self.delete(inner)?;
                let chain = self.f().chains.pop().expect("chain");
                let end = self.emit(Op::Jump(0));
                if !chain.tramps.is_empty() {
                    for tramp in chain.tramps {
                        self.patch(tramp);
                    }
                    self.emit(Op::Swap);
                    self.emit(Op::Pop);
                }
                for exit in chain.exits {
                    self.patch(exit);
                }
                self.emit(Op::Pop);
                self.emit(Op::True);
                self.patch(end);
            }
            ExprKind::Ident(name) => {
                let with = self.delete_checks(name)?;
                if matches!(self.resolve(name), None | Some((Place::Global, ..))) {
                    let index = self.name(name)?;
                    self.emit(Op::DeleteGlobal(index));
                } else {
                    self.emit(Op::False);
                }
                self.patch_all(with);
            }
            _ => {
                self.expr(operand)?;
                self.emit(Op::Pop);
                self.emit(Op::True);
            }
        }
        Ok(())
    }
    /** `++` and `--` on names and members. */
    fn update(&mut self, increment: bool, prefix: bool, target: &Expr) -> EngineResult<()> {
        let step = if increment { Op::Inc } else { Op::Dec };
        match &target.kind {
            ExprKind::Ident(name) => {
                let base = self.ref_name(name, target.pos)?;
                self.load_ref(name, target.pos, base)?;
                self.emit(Op::ToNumeric);
                if !prefix {
                    self.emit(Op::Dup);
                    if base {
                        self.emit(Op::Rot3);
                    }
                }
                self.emit(step);
                self.assign_ref(name, target.pos, base)?;
                if !prefix {
                    self.emit(Op::Pop);
                }
            }
            ExprKind::Member(object, prop, _) if matches!(object.kind, ExprKind::Super) => {
                return self.super_update(prop, increment, prefix, target.pos);
            }
            ExprKind::Member(object, prop, _) => {
                self.expr(object)?;
                match prop {
                    MemberProp::Computed(key) => {
                        self.element_key(key)?;
                        self.emit(Op::Dup2);
                        self.emit(Op::GetElem);
                        self.emit(Op::ToNumeric);
                        if !prefix {
                            self.emit(Op::Dup);
                            self.emit(Op::Rot4);
                        }
                        self.emit(step);
                        self.emit(Op::SetElem);
                    }
                    prop => {
                        let index = match prop {
                            MemberProp::Name(name) => self.name(name)?,
                            MemberProp::Private(name) => self.private_name(name)?,
                            MemberProp::Computed(_) => unreachable!("handled above"),
                        };
                        self.emit(Op::Dup);
                        self.emit(Op::GetProp(index));
                        self.emit(Op::ToNumeric);
                        if !prefix {
                            self.emit(Op::Dup);
                            self.emit(Op::Rot3);
                        }
                        self.emit(step);
                        self.emit(Op::SetProp(index));
                    }
                }
                if !prefix {
                    self.emit(Op::Pop);
                }
            }
            _ => {
                let index = self.name("updates of this target")?;
                self.emit(Op::Unsupported(index));
            }
        }
        Ok(())
    }
    /** Plain, compound and logical assignment. */
    fn assign(&mut self, operator: AssignOp, target: &Pattern, value: &Expr) -> EngineResult<()> {
        match (operator, target) {
            (AssignOp::Assign, Pattern::Ident(name, pos)) => {
                let base = self.ref_name(name, *pos)?;
                self.expr(value)?;
                self.assign_ref(name, *pos, base)
            }
            (AssignOp::Assign, Pattern::Expr(member)) => {
                self.member_assign(member, |c| c.expr(value))
            }
            (AssignOp::Assign, pattern) => {
                self.expr(value)?;
                // V8's printer reaches the value of an assigned object pattern but not of an
                // assigned array pattern, whose iteration is positioned at the assignment.
                let source = if matches!(pattern, Pattern::Array(..)) {
                    Source::Parameter
                } else {
                    Source::Expr(value)
                };
                self.assign_pattern(pattern, source)
            }
            (AssignOp::Binary(op), Pattern::Ident(name, pos)) => {
                let base = self.ref_name(name, *pos)?;
                self.load_ref(name, *pos, base)?;
                self.expr(value)?;
                self.emit(binary(op));
                self.assign_ref(name, *pos, base)
            }
            (AssignOp::Binary(op), Pattern::Expr(member)) => {
                self.compound_member(member, op, value)
            }
            (AssignOp::Logical(op), Pattern::Ident(name, pos)) => {
                let base = self.ref_name(name, *pos)?;
                self.load_ref(name, *pos, base)?;
                let skip = self.emit(logical(op));
                self.expr(value)?;
                self.assign_ref(name, *pos, base)?;
                self.end_logical_ref(skip, base);
                Ok(())
            }
            (AssignOp::Logical(op), Pattern::Expr(member)) => {
                self.logical_member(member, op, value)
            }
            _ => Err(self.error(value.pos, "Invalid assignment target")),
        }
    }
    /** `obj.key = value` with the value produced by `value`. */
    pub(super) fn member_assign(
        &mut self,
        member: &Expr,
        value: impl FnOnce(&mut Self) -> EngineResult<()>,
    ) -> EngineResult<()> {
        let ExprKind::Member(object, prop, _) = &member.kind else {
            return Err(self.error(member.pos, "Invalid assignment target"));
        };
        let is_super = matches!(object.kind, ExprKind::Super);
        if is_super {
            self.load_this()?;
            self.super_update_key(prop, member.pos)?;
            self.load_home()?;
            self.emit(Op::CaptureSuperBase);
            self.emit(Op::Swap);
            value(self)?;
            self.at(member.pos);
            self.emit(Op::WriteSuperReference);
            return Ok(());
        } else {
            self.expr(object)?;
        }
        match prop {
            MemberProp::Computed(key) => {
                self.expr(key)?;
                value(self)?;
                self.at(member.pos);
                self.emit(if is_super {
                    Op::SetSuperElem
                } else {
                    Op::SetElem
                });
            }
            MemberProp::Name(name) => {
                let index = self.name(name)?;
                value(self)?;
                self.at(member.pos);
                self.emit(if is_super {
                    Op::SetSuper(index)
                } else {
                    Op::SetProp(index)
                });
            }
            MemberProp::Private(name) => {
                let index = self.private_name(name)?;
                value(self)?;
                self.at(member.pos);
                self.emit(Op::SetProp(index));
            }
        }
        Ok(())
    }
    /** Push the home object of the method `super` refers to. */
    pub(super) fn load_home(&mut self) -> EngineResult<()> {
        if self.resolve("%home").is_some() {
            self.load_name(&Name::from("%home"), Pos::default())
        } else {
            self.emit(Op::LoadHome);
            Ok(())
        }
    }
    /** `obj.key op= value`. */
    fn compound_member(&mut self, member: &Expr, op: BinaryOp, value: &Expr) -> EngineResult<()> {
        let ExprKind::Member(object, prop, _) = &member.kind else {
            return Err(self.error(member.pos, "Invalid assignment target"));
        };
        if matches!(object.kind, ExprKind::Super) {
            self.load_this()?;
            self.super_update_key(prop, member.pos)?;
            self.load_home()?;
            self.emit(Op::CaptureSuperBase);
            self.emit(Op::Swap);
            self.emit(Op::ReadSuperReference);
            self.expr(value)?;
            self.emit(binary(op));
            self.at(member.pos);
            self.emit(Op::WriteSuperReference);
            return Ok(());
        }
        self.expr(object)?;
        match prop {
            MemberProp::Computed(key) => {
                self.element_key(key)?;
                self.emit(Op::Dup2);
                self.emit(Op::GetElem);
                self.expr(value)?;
                self.emit(binary(op));
                self.at(member.pos);
                self.emit(Op::SetElem);
            }
            prop => {
                let index = match prop {
                    MemberProp::Name(name) => self.name(name)?,
                    MemberProp::Private(name) => self.private_name(name)?,
                    MemberProp::Computed(_) => unreachable!("handled above"),
                };
                self.emit(Op::Dup);
                self.emit(Op::GetProp(index));
                self.expr(value)?;
                self.emit(binary(op));
                self.at(member.pos);
                self.emit(Op::SetProp(index));
            }
        }
        Ok(())
    }
    /** `obj.key ||= value` and friends: the store happens only when needed. */
    fn logical_member(&mut self, member: &Expr, op: LogicalOp, value: &Expr) -> EngineResult<()> {
        let ExprKind::Member(object, prop, _) = &member.kind else {
            return Err(self.error(member.pos, "Invalid assignment target"));
        };
        if matches!(object.kind, ExprKind::Super) {
            let mark = self.f().registers;
            let old = self.temp()?;
            self.load_this()?;
            self.super_update_key(prop, member.pos)?;
            self.load_home()?;
            self.emit(Op::CaptureSuperBase);
            self.emit(Op::Swap);
            self.emit(Op::ReadSuperReference);
            let skip = self.emit(logical(op));
            self.expr(value)?;
            self.at(member.pos);
            self.emit(Op::WriteSuperReference);
            let done = self.emit(Op::Jump(0));
            self.patch(skip);
            self.emit(Op::SetReg(old));
            for _ in 0..4 {
                self.emit(Op::Pop);
            }
            self.emit(Op::GetReg(old));
            self.patch(done);
            self.release(mark);
            return Ok(());
        }
        let mark = self.f().registers;
        let holder = self.temp()?;
        let key = self.temp()?;
        self.expr(object)?;
        self.emit(Op::SetReg(holder));
        match prop {
            MemberProp::Computed(expr) => {
                self.element_key(expr)?;
                self.emit(Op::SetReg(key));
                self.emit(Op::GetElem);
            }
            MemberProp::Name(name) => {
                let index = self.name(name)?;
                self.emit(Op::GetProp(index));
            }
            MemberProp::Private(name) => {
                let index = self.private_name(name)?;
                self.emit(Op::GetProp(index));
            }
        }
        let skip = self.emit(logical(op));
        self.emit(Op::GetReg(holder));
        match prop {
            MemberProp::Computed(_) => {
                self.emit(Op::GetReg(key));
                self.expr(value)?;
                self.emit(Op::SetElem);
            }
            MemberProp::Name(name) => {
                let index = self.name(name)?;
                self.expr(value)?;
                self.emit(Op::SetProp(index));
            }
            MemberProp::Private(name) => {
                let index = self.private_name(name)?;
                self.expr(value)?;
                self.emit(Op::SetProp(index));
            }
        }
        self.patch(skip);
        self.release(mark);
        Ok(())
    }
}

/** Short arrays build in one instruction; longer ones push element by element. */
const ARRAY_LITERAL_BATCH: usize = 256;

/** An optional chain's pending exits. */
#[derive(Default)]
pub(super) struct Chain {
    /// Jumps that leave undefined on the stack.
    pub exits: Vec<usize>,
    /// Jumps from optional method calls that also leave the object beneath.
    pub tramps: Vec<usize>,
}

/** Whether a list spreads. */
fn spreads(elements: &[Element]) -> bool {
    elements
        .iter()
        .any(|element| matches!(element, Element::Spread(_) | Element::Hole))
}

/** Argument count as an operand. */
fn count(elements: &[Element]) -> EngineResult<Slot> {
    Slot::try_from(elements.len())
        .map_err(|_| EngineError::limit("V9 call arguments", Slot::MAX as usize))
}

/** The instruction for a binary operator. */
pub(super) fn binary(operator: BinaryOp) -> Op {
    match operator {
        BinaryOp::Add => Op::Add,
        BinaryOp::Sub => Op::Sub,
        BinaryOp::Mul => Op::Mul,
        BinaryOp::Div => Op::Div,
        BinaryOp::Mod => Op::Mod,
        BinaryOp::Exp => Op::Exp,
        BinaryOp::Shl => Op::Shl,
        BinaryOp::Shr => Op::Shr,
        BinaryOp::UShr => Op::UShr,
        BinaryOp::BitAnd => Op::BitAnd,
        BinaryOp::BitOr => Op::BitOr,
        BinaryOp::BitXor => Op::BitXor,
        BinaryOp::Eq => Op::Eq,
        BinaryOp::Ne => Op::Ne,
        BinaryOp::StrictEq => Op::StrictEq,
        BinaryOp::StrictNe => Op::StrictNe,
        BinaryOp::Lt => Op::Lt,
        BinaryOp::Le => Op::Le,
        BinaryOp::Gt => Op::Gt,
        BinaryOp::Ge => Op::Ge,
        BinaryOp::In => Op::In,
        BinaryOp::InstanceOf => Op::InstanceOf,
    }
}

/** The short-circuit jump for a logical operator (patched later). */
fn logical(operator: LogicalOp) -> Op {
    match operator {
        LogicalOp::And => Op::JumpIfFalseKeep(0),
        LogicalOp::Or => Op::JumpIfTrueKeep(0),
        LogicalOp::Nullish => Op::JumpIfNotNullishKeep(0),
    }
}

#[cfg(feature = "conformance")]
impl Compiler {
    /** Optimizer intrinsics evaluate every argument for its side effects and keep only what V8
    answers without a JIT: hints are `undefined`, tier predicates are constants, `%DebugPrint` and
    `%CompileBaseline` return their subject, and `%GetOptimizationStatus` asks the VM. */
    fn optimizer_intrinsic(&mut self, intrinsic: u8, args: &[Element]) -> EngineResult<()> {
        for (index, arg) in args.iter().enumerate() {
            if let Element::Value(value) = arg {
                self.expr(value)?;
                if index > 0 || !matches!(intrinsic, 68 | 71) {
                    self.emit(Op::Pop);
                }
            }
        }
        let answer = match intrinsic {
            67 => Op::Undefined,
            68 => Op::NativeIntrinsic(68),
            69 => Op::False,
            70 => Op::True,
            _ => return Ok(()),
        };
        self.emit(answer);
        Ok(())
    }
}

/** Duplicate prototype setters are early errors only in literals, never cover patterns. */
pub(super) fn literal_prototypes(props: &[PropDef]) -> EngineResult<()> {
    let mut seen = false;
    for prop in props {
        if let PropDef::Value(PropKey::Name(name), value) = prop
            && &**name == "__proto__"
        {
            if seen {
                return Err(errors::syntax(
                    value.pos.location(),
                    "Duplicate __proto__ fields are not allowed in object literals",
                ));
            }
            seen = true;
        }
    }
    Ok(())
}
