/** Destructuring: binding declarations and assignment targets share one lowering. */
use super::*;
use crate::v9::syntax::{ExprKind, MemberProp, Pattern, PropKey};

/** How a pattern stores each name it reaches. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Store {
    /** Initialize a declared lexical binding. */
    Initialize,
    /** Ordinary assignment (var bindings, existing references). */
    Assign,
}

/** A member assignment target whose base (and computed key) sit in registers. */
struct Target {
    base: Slot,
    key: TargetKey,
    pos: Pos,
}

/** The property a `Target` names: a constant or private name index, or a key register. */
enum TargetKey {
    Name(u32),
    Computed(Slot),
}

impl Compiler {
    /** Bind the value on the stack (kept) to a declaration pattern. */
    pub(super) fn bind_pattern(&mut self, pattern: &Pattern, initialize: bool) -> EngineResult<()> {
        self.pattern(
            pattern,
            if initialize {
                Store::Initialize
            } else {
                Store::Assign
            },
        )
    }
    /** Assign the value on the stack (kept) to an assignment pattern. */
    pub(super) fn assign_pattern(&mut self, pattern: &Pattern) -> EngineResult<()> {
        self.pattern(pattern, Store::Assign)
    }
    /** One pattern level. */
    fn pattern(&mut self, pattern: &Pattern, store: Store) -> EngineResult<()> {
        self.pattern_into(pattern, store, None)
    }
    /** Evaluate a member target's reference into registers before its value is read: spec
    order is lref, then GetV or IteratorStep, then PutValue (13.15.5.5, 13.15.5.6). Super
    targets and nested patterns keep the late evaluation. */
    fn early_target(&mut self, pattern: &Pattern) -> EngineResult<Option<Target>> {
        let member = match pattern {
            Pattern::Expr(member) => member,
            Pattern::Default(inner, _) => match inner.as_ref() {
                Pattern::Expr(member) => member,
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        let ExprKind::Member(object, prop, _) = &member.kind else {
            return Ok(None);
        };
        if matches!(object.kind, ExprKind::Super) {
            return Ok(None);
        }
        let base = self.temp()?;
        self.expr(object)?;
        self.emit(Op::SetReg(base));
        self.emit(Op::Pop);
        let key = match prop {
            MemberProp::Computed(key) => {
                let slot = self.temp()?;
                self.expr(key)?;
                self.emit(Op::SetReg(slot));
                self.emit(Op::Pop);
                TargetKey::Computed(slot)
            }
            MemberProp::Name(name) => TargetKey::Name(self.name(name)?),
            MemberProp::Private(name) => TargetKey::Name(self.private_name(name)?),
        };
        Ok(Some(Target {
            base,
            key,
            pos: member.pos,
        }))
    }
    /** PutValue into a reference evaluated by `early_target`; the value stays on the stack. */
    fn store_target(&mut self, target: Target) -> EngineResult<()> {
        let value = self.temp()?;
        self.emit(Op::SetReg(value));
        self.emit(Op::GetReg(target.base));
        let op = match target.key {
            TargetKey::Computed(slot) => {
                self.emit(Op::GetReg(slot));
                self.emit(Op::ToElementKey);
                Op::SetElem
            }
            TargetKey::Name(index) => Op::SetProp(index),
        };
        self.emit(Op::GetReg(value));
        self.at(target.pos);
        self.emit(op);
        self.emit(Op::Pop);
        Ok(())
    }
    /** One pattern level, storing into `target` when its reference was evaluated early. */
    fn pattern_into(
        &mut self,
        pattern: &Pattern,
        store: Store,
        target: Option<Target>,
    ) -> EngineResult<()> {
        match pattern {
            Pattern::Expr(_) if target.is_some() => self.store_target(target.expect("target")),
            Pattern::Ident(name, pos) => match store {
                Store::Initialize => self.initialize(name, *pos),
                Store::Assign => self.assign_name(name, *pos),
            },
            Pattern::Expr(member) => {
                // value -> value: evaluate the target, then store a copy of the value.
                let mark = self.f().registers;
                let value = self.temp()?;
                self.emit(Op::SetReg(value));
                self.member_assign(member, |compiler| {
                    compiler.emit(Op::GetReg(value));
                    Ok(())
                })?;
                self.emit(Op::Pop);
                self.release(mark);
                Ok(())
            }
            Pattern::Default(inner, default) => {
                self.emit(Op::Dup);
                self.emit(Op::Undefined);
                self.emit(Op::StrictEq);
                let skip = self.emit(Op::JumpIfFalse(0));
                self.emit(Op::Pop);
                self.expr(default)?;
                self.patch(skip);
                self.pattern_into(inner, store, target)
            }
            Pattern::Object(props, rest, pos) => {
                self.object_pattern(props, rest.as_deref(), *pos, store)
            }
            Pattern::Array(items, rest, pos) => {
                self.array_pattern(items, rest.as_deref(), *pos, store)
            }
        }
    }
    /** `{a, b: c, [k]: d, ...rest}` reads properties from the value. */
    fn object_pattern(
        &mut self,
        props: &[crate::v9::syntax::PatProp],
        rest: Option<&Pattern>,
        pos: Pos,
        store: Store,
    ) -> EngineResult<()> {
        self.at(pos);
        let mark = self.f().registers;
        let keys: Vec<Slot> = if rest.is_some() {
            (0..props.len())
                .map(|_| self.temp())
                .collect::<EngineResult<_>>()?
        } else {
            Vec::new()
        };
        // Destructuring null or undefined throws, even for an empty pattern; CopyRest raises
        // that error, and runs only for a nullish value so no getter is read twice.
        self.emit(Op::Dup);
        let ok = self.emit(Op::JumpIfNotNullishKeep(0));
        self.emit(Op::Dup);
        self.emit(Op::CopyRest(0));
        self.patch(ok);
        self.emit(Op::Pop);
        for (index, prop) in props.iter().enumerate() {
            self.emit(Op::Dup);
            let target = match &prop.key {
                PropKey::Computed(key) => {
                    self.expr(key)?;
                    self.emit(Op::ToPropertyKey);
                    if let Some(slot) = keys.get(index) {
                        self.emit(Op::SetReg(*slot));
                    }
                    let target = self.early_target(&prop.value)?;
                    self.emit(Op::GetElem);
                    target
                }
                key => {
                    let name = self.key_index(key)?;
                    if let Some(slot) = keys.get(index) {
                        self.emit(Op::String(name));
                        self.emit(Op::SetReg(*slot));
                        self.emit(Op::Pop);
                    }
                    let target = self.early_target(&prop.value)?;
                    self.emit(Op::GetProp(name));
                    target
                }
            };
            self.pattern_into(&prop.value, store, target)?;
            self.emit(Op::Pop);
        }
        if let Some(rest) = rest {
            self.emit(Op::Dup);
            for slot in &keys {
                self.emit(Op::GetReg(*slot));
            }
            self.emit(Op::CopyRest(keys.len() as u32));
            self.pattern(rest, store)?;
            self.emit(Op::Pop);
        }
        self.release(mark);
        Ok(())
    }
    /** `[a, , b = 1, ...rest]` reads from the value's iterator. */
    fn array_pattern(
        &mut self,
        items: &[Option<Pattern>],
        rest: Option<&Pattern>,
        pos: Pos,
        store: Store,
    ) -> EngineResult<()> {
        self.at(pos);
        let mark = self.f().registers;
        let iterator = self.temp()?;
        self.emit(Op::Dup);
        self.emit(Op::GetIterator);
        self.emit(Op::SetReg(iterator));
        self.emit(Op::Pop);
        let closeable = self.temp()?;
        self.pattern_closeable(closeable, true);
        self.pattern_iterator_scope(iterator, closeable, |this| {
            this.array_pattern_items(items, rest, iterator, closeable, store)
        })?;
        self.release(mark);
        Ok(())
    }
    /** Steps cannot request cleanup until their next/done/value observations have succeeded. */
    fn array_pattern_items(
        &mut self,
        items: &[Option<Pattern>],
        rest: Option<&Pattern>,
        iterator: Slot,
        closeable: Slot,
        store: Store,
    ) -> EngineResult<()> {
        for item in items {
            let target = match item {
                Some(item) => self.early_target(item)?,
                None => None,
            };
            self.pattern_closeable(closeable, false);
            self.emit(Op::GetReg(iterator));
            self.emit(Op::IterStep);
            self.pattern_closeable(closeable, true);
            if let Some(item) = item {
                self.pattern_into(item, store, target)?;
            }
            self.emit(Op::Pop);
        }
        if let Some(rest) = rest {
            let target = self.early_target(rest)?;
            self.pattern_closeable(closeable, false);
            self.emit(Op::GetReg(iterator));
            self.emit(Op::IterRest);
            self.pattern_into(rest, store, target)?;
            self.emit(Op::Pop);
        }
        Ok(())
    }
    /** A successful step reopens binding cleanup; rest consumption remains exhausted. */
    fn pattern_closeable(&mut self, slot: Slot, closeable: bool) {
        self.emit(if closeable { Op::True } else { Op::False });
        self.emit(Op::SetReg(slot));
        self.emit(Op::Pop);
    }
}
