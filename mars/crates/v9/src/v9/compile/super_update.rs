/** Super updates retain one evaluated reference across key coercion, numeric conversion and the final write. */
use super::*;
use crate::v9::syntax::MemberProp;

impl Compiler {
    /** The actual receiver precedes key evaluation; the base precedes key coercion, and postfix saves the numeric old value. */
    pub(super) fn super_update(
        &mut self,
        prop: &MemberProp,
        increment: bool,
        prefix: bool,
        pos: Pos,
    ) -> EngineResult<()> {
        let mark = self.f().registers;
        let old = if prefix { None } else { Some(self.temp()?) };
        self.load_this()?;
        self.super_update_key(prop, pos)?;
        self.load_home()?;
        self.emit(Op::CaptureSuperBase);
        self.emit(Op::Swap);
        self.emit(Op::ToPropertyKey);
        self.emit(Op::ReadSuperReference);
        self.emit(Op::ToNumeric);
        if let Some(old) = old {
            self.emit(Op::SetReg(old));
        }
        self.emit(if increment { Op::Inc } else { Op::Dec });
        self.at(pos);
        self.emit(Op::WriteSuperReference);
        if let Some(old) = old {
            self.emit(Op::Pop);
            self.emit(Op::GetReg(old));
        }
        self.release(mark);
        Ok(())
    }

    /** Named and computed references converge before the base is captured, without observing a private super name. */
    fn super_update_key(&mut self, prop: &MemberProp, pos: Pos) -> EngineResult<()> {
        match prop {
            MemberProp::Computed(key) => self.expr(key),
            MemberProp::Name(name) => {
                let index = self.name(name)?;
                self.emit(Op::String(index));
                Ok(())
            }
            MemberProp::Private(_) => Err(self.error(pos, "Unexpected private field")),
        }
    }
}
