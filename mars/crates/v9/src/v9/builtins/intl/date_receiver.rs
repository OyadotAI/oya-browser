/** DateTimeFormat methods require their own native internal brand before observing formatting arguments. */
use super::*;

/** A method's creation-kind is fixed by native registration, never inferred from a borrowed receiver's kind. */
pub(super) fn method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let receiver = crate::v9::platform::host_data(vm, &call.this);
    if !matches!(receiver, Some(HostData::Intl(data)) if data.kind == "DateTimeFormat") {
        return Err(vm.type_error("Intl.DateTimeFormat method called on incompatible receiver"));
    }
    super::method(vm, call)
}
