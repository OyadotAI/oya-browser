/** Math.sumPrecise consumes the iterator protocol and rejects values without numeric coercion. */
use super::exact_sum::ExactSum;
use super::{Call, JsValue, Res, Vm};

/** Iterator counts follow ECMAScript's maximum safe integer bound. */
const MAXIMUM_COUNT: u64 = (1 << f64::MANTISSA_DIGITS) - 1;

/** Iterator errors propagate directly; a rejected element closes while preserving its original error. */
pub(super) fn sum_precise(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let iterator = vm.get_iterator(&call.arg(0))?;
    vm.pin(JsValue::Object(iterator));
    let mut sum = ExactSum::new();
    let mut count = 0;
    while let Some(value) = vm.iter_step(iterator)? {
        vm.charge(1)?;
        let result = add(vm, &mut sum, &mut count, value);
        if let Err(error) = result {
            vm.finish_numeric_iterator(iterator);
            if let Err(crate::v9::vm::Thrown::Fatal(fatal)) = vm.iter_close(iterator) {
                return Err(fatal.into());
            }
            return Err(error);
        }
    }
    Ok(JsValue::Number(sum.finish()))
}

/** Type checks apply even after NaN or infinity; objects and BigInts are never coerced. */
fn add(vm: &mut Vm, sum: &mut ExactSum, count: &mut u64, value: JsValue) -> Res<()> {
    if *count == MAXIMUM_COUNT {
        return Err(vm.range_error("Math.sumPrecise input exceeds the maximum safe integer"));
    }
    *count += 1;
    let JsValue::Number(number) = value else {
        return Err(vm.type_error("Math.sumPrecise requires Number elements"));
    };
    sum.add(number);
    Ok(())
}
