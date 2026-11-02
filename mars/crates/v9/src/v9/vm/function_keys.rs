/** A function's initial own prototype key exists before its lazily allocated prototype object. */
use super::Vm;
use super::data::FunctionKind;
use crate::v9::value::ObjectId;

impl Vm<'_> {
    /** Closure metadata alone determines whether GetOwnProperty and OwnPropertyKeys expose the initial slot. */
    pub(super) fn has_function_prototype(&self, id: ObjectId) -> bool {
        let Some(FunctionKind::Closure { site, .. }) = self
            .heap
            .get(id)
            .and_then(|object| object.function())
            .map(|function| &function.kind)
        else {
            return false;
        };
        self.registry.get(*site).is_some_and(|code| {
            let flags = code.flags;
            flags.generator || !flags.arrow && !flags.method && !flags.asynchronous && !flags.fields
        })
    }
}
