/** The TypeError for a refused property definition: V8's wording names a closed target and an absent key, else a redefinition. */
use super::heap::Key;
use super::property::key_text;
use super::{Thrown, Vm};
use crate::v9::value::ObjectId;

/** Message when a new property cannot be added to a non-extensible object. */
const CLOSED_TARGET: &str = "Cannot define property {}, object is not extensible";
/** Message when an existing property refuses the new descriptor. */
const REDEFINE: &str = "Cannot redefine property: {}";

impl Vm<'_> {
    /** Refusals name the object a proxy finally forwards to, because that is the object that refuses. */
    pub(in crate::v9) fn refused_define(&mut self, id: ObjectId, key: &Key) -> Thrown {
        let target = self.definition_target(id);
        let closed = self
            .heap
            .get(target)
            .is_some_and(|object| !object.extensible);
        let absent = matches!(self.own(target, key), Ok(None));
        let template = if closed && absent {
            CLOSED_TARGET
        } else {
            REDEFINE
        };
        self.type_error(&template.replace("{}", &key_text(key)))
    }

    /** Follows proxy targets with the call-depth bound, so a cyclic chain cannot loop forever. */
    fn definition_target(&self, id: ObjectId) -> ObjectId {
        let mut target = id;
        for _ in 0..self.limits.frames {
            match self.proxy_of(target).and_then(|proxy| proxy.target) {
                Some(next) => target = next,
                None => break,
            }
        }
        target
    }
}
