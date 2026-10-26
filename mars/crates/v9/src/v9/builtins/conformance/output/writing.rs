/** Shell writes preserve conversion order and partial output without granting scripts direct process IO. */
use super::*;
use crate::constants::VM_CONSOLE_BYTES;
use crate::v9::vm::Thrown;

/** Append one bounded UTF8 prefix while preserving scalar boundaries in the native host record. */
pub(super) fn append(output: &mut String, text: &str) {
    let mut end = text
        .len()
        .min(VM_CONSOLE_BYTES.saturating_sub(output.len()));
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    output.push_str(&text[..end]);
}

/** Symbols print their description in d8, including undefined when no description was supplied. */
fn argument(vm: &mut Vm, value: &JsValue) -> Res<String> {
    if let JsValue::Symbol(id) = value {
        return Ok(
            crate::v9::builtins::symbol_description(vm, *id).unwrap_or_else(|| "undefined".into())
        );
    }
    Ok(vm.to_string(value)?.lossy())
}

/** Every argument is converted even after the output limit; throwing conversions retain the already-written prefix. */
pub(super) fn write(
    vm: &mut Vm,
    args: &[JsValue],
    level: &str,
    prefix: &str,
    flush_failure: bool,
) -> Res<()> {
    let mut output = String::new();
    append(&mut output, prefix);
    for (index, value) in args.iter().enumerate() {
        if index > 0 {
            append(&mut output, " ");
        }
        match argument(vm, value) {
            Ok(text) => append(&mut output, &text),
            Err(error) => {
                if (flush_failure || !output.is_empty()) && matches!(error, Thrown::Value(_)) {
                    vm.page.console(level, output);
                }
                return Err(error);
            }
        }
    }
    vm.page.console(level, output);
    Ok(())
}
