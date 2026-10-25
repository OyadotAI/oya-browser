/** Destructuring: binding declarations and assignment targets share one lowering. */
use super::*;
use crate::v9::syntax::{Pattern, PropKey};

/** How a pattern stores each name it reaches. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Store {
    /** Initialize a declared lexical binding. */
    Initialize,
    /** Ordinary assignment (var bindings, existing references). */
    Assign,
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
        match pattern {
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
            Pattern::Default(target, default) => {
                self.emit(Op::Dup);
                self.emit(Op::Undefined);
                self.emit(Op::StrictEq);
                let skip = self.emit(Op::JumpIfFalse(0));
                self.emit(Op::Pop);
                self.expr(default)?;
                self.patch(skip);
                self.pattern(target, store)
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
            match &prop.key {
                PropKey::Computed(key) => {
                    self.expr(key)?;
                    self.emit(Op::ToPropertyKey);
                    if let Some(slot) = keys.get(index) {
                        self.emit(Op::SetReg(*slot));
                    }
                    self.emit(Op::GetElem);
                }
                key => {
                    let name = self.key_index(key)?;
                    if let Some(slot) = keys.get(index) {
                        self.emit(Op::String(name));
                        self.emit(Op::SetReg(*slot));
                        self.emit(Op::Pop);
                    }
                    self.emit(Op::GetProp(name));
                }
            }
            self.pattern(&prop.value, store)?;
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
        for item in items {
            self.emit(Op::GetReg(iterator));
            self.emit(Op::IterStep);
            if let Some(item) = item {
                self.pattern(item, store)?;
            }
            self.emit(Op::Pop);
        }
        if let Some(rest) = rest {
            self.emit(Op::GetReg(iterator));
            self.emit(Op::IterRest);
            self.pattern(rest, store)?;
            self.emit(Op::Pop);
        } else {
            self.emit(Op::GetReg(iterator));
            self.emit(Op::IterClose);
        }
        self.release(mark);
        Ok(())
    }
}
