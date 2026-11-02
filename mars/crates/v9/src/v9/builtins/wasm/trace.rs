/** Structured Wasm CallSites derive names and source identity from the instance's actual immutable module bytes. */
use super::*;

/** Custom-section names are inert UTF-8 labels, while source identity comes from the exact retained bytes. */
pub(in crate::v9) struct WasmTraceMetadata {
    pub function: Option<JsString>,
    pub module: Option<JsString>,
    pub file: String,
}

/** Resolving one frame at allocation time avoids copying debug-name tables into every live activation. */
pub(in crate::v9) fn trace_metadata(
    vm: &Vm,
    instance: ObjectId,
    index: u32,
) -> crate::protocol::EngineResult<WasmTraceMetadata> {
    let invalid =
        || crate::protocol::EngineError::invalid("Invalid active WebAssembly module metadata.");
    let module = get(vm, instance, INSTANCE)
        .and_then(|value| value.object())
        .ok_or_else(invalid)?;
    let buffer = get(vm, module, MODULE)
        .and_then(|value| value.object())
        .ok_or_else(invalid)?;
    let Some(Class::ArrayBuffer(bytes)) = vm.heap.get(buffer).map(|object| &object.class) else {
        return Err(invalid());
    };
    let function = diagnostics::function_names(&bytes.0)
        .remove(&index)
        .map(|name| JsString::from_utf8(&name))
        .transpose()?;
    let module = module_name(&bytes.0)
        .map(|name| JsString::from_utf8(&name))
        .transpose()?;
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes.0);
    let mut file = String::from("wasm://wasm/");
    use std::fmt::Write;
    for byte in digest.as_ref() {
        write!(&mut file, "{byte:02x}").expect("writing String");
    }
    Ok(WasmTraceMetadata {
        function,
        module,
        file,
    })
}

/** The name custom section's module subsection is inspected without granting any runtime capability. */
fn module_name(bytes: &[u8]) -> Option<String> {
    fn read(bytes: &[u8]) -> Result<Option<String>, Error> {
        let mut reader = binary::Reader::new(bytes);
        reader.take(MAGIC.len())?;
        while reader.at < bytes.len() {
            let kind = reader.byte()?;
            let count = reader.u32()? as usize;
            let mut section = binary::Reader::new(reader.take(count)?);
            if kind != 0 || section.name()? != "name" {
                continue;
            }
            while section.at < section.bytes.len() {
                let kind = section.byte()?;
                let count = section.u32()? as usize;
                let mut subsection = binary::Reader::new(section.take(count)?);
                if kind == 0 {
                    return subsection.name().map(Some);
                }
            }
        }
        Ok(None)
    }
    read(bytes).ok().flatten()
}
