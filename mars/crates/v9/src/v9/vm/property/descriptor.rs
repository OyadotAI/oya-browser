/** Descriptor compatibility is shared by ordinary definitions and Proxy invariant checks. */
use super::{CONFIGURABLE, Descriptor, ENUMERABLE, Prop, Slot, WRITABLE};

/** IsCompatiblePropertyDescriptor never mutates the object whose property is being examined. */
pub(in crate::v9::vm) fn compatible(
    extensible: bool,
    descriptor: &Descriptor,
    current: Option<&Prop>,
) -> bool {
    let Some(current) = current else {
        return extensible;
    };
    if current.attrs & CONFIGURABLE != 0 {
        return true;
    }
    if descriptor.configurable == Some(true)
        || descriptor
            .enumerable
            .is_some_and(|value| value != (current.attrs & ENUMERABLE != 0))
    {
        return false;
    }
    slots_compatible(descriptor, current)
}

/** A fixed property cannot change kind, frozen value, getter or setter. */
fn slots_compatible(descriptor: &Descriptor, current: &Prop) -> bool {
    match &current.slot {
        Slot::Data(value) => {
            !descriptor.accessor()
                && (current.attrs & WRITABLE != 0
                    || descriptor.writable != Some(true)
                        && descriptor
                            .value
                            .as_ref()
                            .is_none_or(|next| next.same_value(value)))
        }
        Slot::Accessor(get, set) => {
            descriptor.value.is_none()
                && descriptor.writable.is_none()
                && descriptor.get.is_none_or(|next| next == *get)
                && descriptor.set.is_none_or(|next| next == *set)
        }
    }
}
