/** Unicode normalization preserves JavaScript's lone UTF-16 surrogates and bounds expanded output. */
use super::{Call, JsString, JsValue, Res, Vm, this_string};
use crate::constants::JS_STRING_UNITS;
use crate::protocol::EngineError;
use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};

/** The four normalization algorithms accepted by ECMAScript. */
enum Form {
    Nfc,
    Nfd,
    Nfkc,
    Nfkd,
}

impl Form {
    /** Select the algorithm after the receiver has been coerced, even for empty strings. */
    fn read(vm: &mut Vm, value: JsValue) -> Res<Self> {
        if matches!(value, JsValue::Undefined) {
            return Ok(Self::Nfc);
        }
        match vm.to_string(&value)?.lossy().as_str() {
            "NFC" => Ok(Self::Nfc),
            "NFD" => Ok(Self::Nfd),
            "NFKC" => Ok(Self::Nfkc),
            "NFKD" => Ok(Self::Nfkd),
            _ => {
                Err(vm.range_error("The normalization form should be one of NFC, NFD, NFKC, NFKD."))
            }
        }
    }

    /** ICU processes scalar values; ill-formed UTF-16 boundaries are handled by the caller. */
    fn segment(&self, vm: &mut Vm, chars: &[char], output: &mut Vec<u16>) -> Res<()> {
        match self {
            Self::Nfc => append(
                vm,
                output,
                ComposingNormalizerBorrowed::new_nfc().normalize_iter(chars.iter().copied()),
            ),
            Self::Nfd => append(
                vm,
                output,
                DecomposingNormalizerBorrowed::new_nfd().normalize_iter(chars.iter().copied()),
            ),
            Self::Nfkc => append(
                vm,
                output,
                ComposingNormalizerBorrowed::new_nfkc().normalize_iter(chars.iter().copied()),
            ),
            Self::Nfkd => append(
                vm,
                output,
                DecomposingNormalizerBorrowed::new_nfkd().normalize_iter(chars.iter().copied()),
            ),
        }
    }
}

/** Check each expansion before storing it so compatibility decomposition cannot escape quotas. */
fn append(vm: &mut Vm, output: &mut Vec<u16>, chars: impl Iterator<Item = char>) -> Res<()> {
    for character in chars {
        vm.charge(1)?;
        let mut buffer = [0; char::MAX_LEN_UTF16];
        append_units(output, character.encode_utf16(&mut buffer))?;
    }
    Ok(())
}

/** Lone surrogates and scalar expansions consume the same UTF-16 storage allowance. */
fn append_units(output: &mut Vec<u16>, units: &[u16]) -> Res<()> {
    if output.len().saturating_add(units.len()) > JS_STRING_UNITS {
        return Err(EngineError::limit("JavaScript string UTF-16 units", JS_STRING_UNITS).into());
    }
    output.extend_from_slice(units);
    Ok(())
}

/** String.prototype.normalize is generic and treats each lone surrogate as a normalization boundary. */
pub(super) fn normalize(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = this_string(vm, &call)?;
    let form = Form::read(vm, call.arg(0))?;
    let mut output = Vec::with_capacity(text.len());
    let mut segment = Vec::new();
    for decoded in char::decode_utf16(text.as_units().iter().copied()) {
        vm.charge(1)?;
        match decoded {
            Ok(character) => segment.push(character),
            Err(error) => {
                form.segment(vm, &segment, &mut output)?;
                segment.clear();
                append_units(&mut output, &[error.unpaired_surrogate()])?;
            }
        }
    }
    form.segment(vm, &segment, &mut output)?;
    Ok(JsValue::String(JsString::from_units(output)?))
}
