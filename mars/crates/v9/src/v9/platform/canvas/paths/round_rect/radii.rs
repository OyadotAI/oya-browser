/** Bounded union conversion preserves per-item iterator effects and DOMPointInit member order. */
use super::*;
use crate::v9::vm::{heap::Key, realm::symbol};

/** Only four converted values can be geometrically valid; retain the count separately for RangeError. */
const CORNERS: usize = 4;
/** Invalid or infinite sequences cannot perform unlimited native conversion work before their RangeError. */
const INPUTS: usize = 1024;

/** Retaining only useful radii bounds memory while allowing full conversion of short invalid sequences. */
pub(super) struct Radii {
    /** At most four radii are retained; later values are converted only for their required observable effects. */
    pub values: Vec<[f64; 2]>,
    /** The full admitted sequence length distinguishes invalid lists from valid corner shorthand. */
    pub count: usize,
}

/** Objects select iterable or dictionary conversion; primitive values select the numeric member of the union. */
pub(super) fn convert(vm: &mut Vm, value: &JsValue) -> Res<Radii> {
    if value.object().is_some() {
        let method = vm.get_method(value, &Key::Sym(symbol::ITERATOR))?;
        if let Some(method) = method {
            return sequence(vm, value, method);
        }
    }
    Ok(Radii {
        values: vec![radius(vm, value)?],
        count: 1,
    })
}

/** Convert each yielded item before advancing again, and close the iterator when item conversion fails. */
fn sequence(vm: &mut Vm, value: &JsValue, method: JsValue) -> Res<Radii> {
    let iterator = vm.iterator_from_method(value, method)?;
    vm.pin(JsValue::Object(iterator));
    let mut radii = Radii {
        values: Vec::new(),
        count: 0,
    };
    while let Some(value) = vm.iter_step(iterator)? {
        vm.pin(value.clone());
        if radii.count == INPUTS {
            let error = quota(vm);
            let _ = vm.iter_close(iterator);
            return Err(error);
        }
        let value = match radius(vm, &value) {
            Ok(value) => value,
            Err(error) => {
                let _ = vm.iter_close(iterator);
                return Err(error);
            }
        };
        if radii.count < CORNERS {
            radii.values.push(value);
        }
        radii.count += 1;
    }
    Ok(radii)
}

/** Null/undefined dictionaries have default zero axes; a numeric primitive gives equal axes. */
fn radius(vm: &mut Vm, value: &JsValue) -> Res<[f64; 2]> {
    if value.nullish() {
        return Ok([0.0; 2]);
    }
    if value.object().is_none() {
        let number = vm.to_number(value)?;
        return Ok([number; 2]);
    }
    let mut axes = [0.0; 2];
    for name in ["w", "x", "y", "z"] {
        let member = vm.get(value, &Key::from(name))?;
        let default = if name == "w" { 1.0 } else { 0.0 };
        let number = if matches!(member, JsValue::Undefined) {
            default
        } else {
            vm.to_number(&member)?
        };
        if name == "x" {
            axes[0] = number;
        }
        if name == "y" {
            axes[1] = number;
        }
    }
    Ok(axes)
}
