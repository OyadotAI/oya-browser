/**
 * `navigator.plugins` and `navigator.mimeTypes` as Chrome 152 reports them on a desktop: the
 * five PDF viewer entries, each with the two PDF MIME types, in PluginArray, Plugin,
 * MimeTypeArray and MimeType objects. Chrome has shipped exactly this fixed list since 94, and
 * an empty list beside a Chrome User-Agent is what headless and automation checks look for.
 */
use crate::v9::value::JsValue;
use crate::v9::vm::heap::{ENUMERABLE, Key};
use crate::v9::vm::realm::{Builder, string};
use crate::v9::vm::{Call, Res, Vm};

/** Chrome's plugin names, in Chrome's order. */
const PLUGINS: [&str; 5] = [
    "PDF Viewer",
    "Chrome PDF Viewer",
    "Chromium PDF Viewer",
    "Microsoft Edge PDF Viewer",
    "WebKit built-in PDF",
];
/** The MIME types every entry handles. */
const MIME_TYPES: [&str; 2] = ["application/pdf", "text/pdf"];
/** Every entry's filename and description. */
const FILENAME: &str = "internal-pdf-viewer";
const DESCRIPTION: &str = "Portable Document Format";
const SUFFIXES: &str = "pdf";

/** The four interfaces, the fixed entries, and the two navigator attributes. */
pub(super) fn install(b: &mut Builder, navigator_proto: u32) {
    let [plugin_array, mime_type_array, plugin_proto, mime_type_proto] =
        ["PluginArray", "MimeTypeArray", "Plugin", "MimeType"].map(|name| {
            let proto = b.object(Some(b.ids.object_prototype));
            super::chrome_env::interface(b, name, proto);
            proto
        });
    for proto in [plugin_array, mime_type_array, plugin_proto] {
        b.host_method(proto, "item", 1, item);
        b.host_method(proto, "namedItem", 1, named_item);
    }
    b.host_method(plugin_array, "refresh", 0, |_, _| Ok(JsValue::Undefined));
    let plugins = b.object(Some(plugin_array));
    let mime_types = b.object(Some(mime_type_array));
    for (index, name) in PLUGINS.iter().enumerate() {
        let plugin = plugin_entry(b, plugin_proto, mime_type_proto, name, index, mime_types);
        b.value(
            plugins,
            &index.to_string(),
            JsValue::Object(plugin),
            ENUMERABLE,
        );
        b.alias(plugins, name, plugin);
    }
    b.value(plugins, "length", JsValue::Number(PLUGINS.len() as f64), 0);
    b.value(
        mime_types,
        "length",
        JsValue::Number(MIME_TYPES.len() as f64),
        0,
    );
    b.alias(navigator_proto, "plugins", plugins);
    b.alias(navigator_proto, "mimeTypes", mime_types);
}

/** One plugin with its MIME types; the first plugin's types also fill `navigator.mimeTypes`. */
fn plugin_entry(
    b: &mut Builder,
    plugin_proto: u32,
    mime_type_proto: u32,
    name: &str,
    index: usize,
    mime_types: u32,
) -> u32 {
    let plugin = b.object(Some(plugin_proto));
    b.value(plugin, "name", string(name), ENUMERABLE);
    b.value(plugin, "filename", string(FILENAME), ENUMERABLE);
    b.value(plugin, "description", string(DESCRIPTION), ENUMERABLE);
    b.value(
        plugin,
        "length",
        JsValue::Number(MIME_TYPES.len() as f64),
        0,
    );
    for (position, mime) in MIME_TYPES.iter().enumerate() {
        let mime_type = b.object(Some(mime_type_proto));
        b.value(mime_type, "type", string(mime), ENUMERABLE);
        b.value(mime_type, "suffixes", string(SUFFIXES), ENUMERABLE);
        b.value(mime_type, "description", string(DESCRIPTION), ENUMERABLE);
        b.alias(mime_type, "enabledPlugin", plugin);
        b.value(
            plugin,
            &position.to_string(),
            JsValue::Object(mime_type),
            ENUMERABLE,
        );
        b.alias(plugin, mime, mime_type);
        if index == 0 {
            b.value(
                mime_types,
                &position.to_string(),
                JsValue::Object(mime_type),
                ENUMERABLE,
            );
            b.alias(mime_types, mime, mime_type);
        }
    }
    plugin
}

/** `item(index)`: the entry at that index, or null. */
fn item(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let index = vm.to_number(&call.arg(0))?;
    if index < 0.0 || index.fract() != 0.0 {
        return Ok(JsValue::Null);
    }
    let key = Key::from(&(index as u64).to_string());
    Ok(present(vm.get(&call.this, &key)?))
}

/** `namedItem(name)`: the entry of that name or type, or null. */
fn named_item(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let key = Key::Str(vm.to_string(&call.arg(0))?);
    Ok(present(vm.get(&call.this, &key)?))
}

/** Missing entries read as null, as the WebIDL getters say. */
fn present(value: JsValue) -> JsValue {
    match value {
        JsValue::Undefined => JsValue::Null,
        other => other,
    }
}
