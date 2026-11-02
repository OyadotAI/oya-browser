/** The order V8's bootstrapper gives every intrinsic's own properties. Pages fingerprint the
engine by listing `Object.getOwnPropertyNames(Math)` and friends, so after every builtin is
installed this pass puts each intrinsic's keys in the order Chrome prints, taken from V8's
`src/init/bootstrapper.cc` and confirmed against node and headless Chrome. */
use super::*;
use crate::v9::vm::realm::{Static, symbol};

/** Every constructor, prototype and namespace whose own-key order a page can observe. */
pub(super) fn install(b: &mut Builder) {
    constructors(b);
    prototypes(b);
    namespaces(b);
    typed_arrays(b);
    iteration(b);
    intl(b);
    globals(b);
}

/** The global constructors whose statics V8 installs in a different order from V9's tables. */
fn constructors(b: &mut Builder) {
    order(
        b,
        global(b, "Object"),
        &[
            "length",
            "name",
            "prototype",
            "assign",
            "getOwnPropertyDescriptor",
            "getOwnPropertyDescriptors",
            "getOwnPropertyNames",
            "getOwnPropertySymbols",
            "hasOwn",
            "is",
            "preventExtensions",
            "seal",
            "create",
            "defineProperties",
            "defineProperty",
            "freeze",
            "getPrototypeOf",
            "setPrototypeOf",
            "isExtensible",
            "isFrozen",
            "isSealed",
            "keys",
            "entries",
            "fromEntries",
            "values",
            "groupBy",
        ],
        &[],
    );
    order(
        b,
        global(b, "Array"),
        &[
            "length",
            "name",
            "prototype",
            "isArray",
            "from",
            "fromAsync",
            "of",
        ],
        &[],
    );
    order(
        b,
        global(b, "Number"),
        &[
            "length",
            "name",
            "prototype",
            "isFinite",
            "isInteger",
            "isNaN",
            "isSafeInteger",
            "parseFloat",
            "parseInt",
            "MAX_VALUE",
            "MIN_VALUE",
            "NaN",
            "NEGATIVE_INFINITY",
            "POSITIVE_INFINITY",
            "MAX_SAFE_INTEGER",
            "MIN_SAFE_INTEGER",
            "EPSILON",
        ],
        &[],
    );
    order(
        b,
        global(b, "Symbol"),
        &[
            "length",
            "name",
            "prototype",
            "for",
            "keyFor",
            "asyncIterator",
            "hasInstance",
            "isConcatSpreadable",
            "iterator",
            "match",
            "matchAll",
            "replace",
            "search",
            "species",
            "split",
            "toPrimitive",
            "toStringTag",
            "unscopables",
            "dispose",
            "asyncDispose",
        ],
        &[],
    );
    order(
        b,
        global(b, "RegExp"),
        &[
            "length",
            "name",
            "prototype",
            "input",
            "$_",
            "lastMatch",
            "$&",
            "lastParen",
            "$+",
            "leftContext",
            "$`",
            "rightContext",
            "$'",
            "$1",
            "$2",
            "$3",
            "$4",
            "$5",
            "$6",
            "$7",
            "$8",
            "$9",
            "escape",
        ],
        &[],
    );
    order(
        b,
        global(b, "Promise"),
        &[
            "length",
            "name",
            "prototype",
            "all",
            "allSettled",
            "any",
            "race",
            "resolve",
            "reject",
            "withResolvers",
            "try",
        ],
        &[],
    );
    order(b, global(b, "Proxy"), &["length", "name", "revocable"], &[]);
    order(
        b,
        global(b, "Uint8Array"),
        &[
            "length",
            "name",
            "prototype",
            "BYTES_PER_ELEMENT",
            "fromBase64",
            "fromHex",
        ],
        &[],
    );
    order(
        b,
        global(b, "Iterator"),
        &["length", "name", "prototype", "from", "concat"],
        &[],
    );
}

/** The prototypes of the global constructors. */
fn prototypes(b: &mut Builder) {
    order(
        b,
        b.ids.object_prototype,
        &[
            "constructor",
            "__defineGetter__",
            "__defineSetter__",
            "hasOwnProperty",
            "__lookupGetter__",
            "__lookupSetter__",
            "isPrototypeOf",
            "propertyIsEnumerable",
            "toString",
            "valueOf",
            "__proto__",
            "toLocaleString",
        ],
        &[],
    );
    order(
        b,
        b.ids.function_prototype,
        &[
            "length",
            "name",
            "constructor",
            "apply",
            "bind",
            "call",
            "toString",
            "arguments",
            "caller",
        ],
        &[],
    );
    order(
        b,
        b.ids.array_prototype,
        &[
            "length",
            "constructor",
            "at",
            "concat",
            "copyWithin",
            "fill",
            "find",
            "findIndex",
            "findLast",
            "findLastIndex",
            "lastIndexOf",
            "pop",
            "push",
            "reverse",
            "shift",
            "unshift",
            "slice",
            "sort",
            "splice",
            "includes",
            "indexOf",
            "join",
            "keys",
            "entries",
            "values",
            "forEach",
            "filter",
            "flat",
            "flatMap",
            "map",
            "every",
            "some",
            "reduce",
            "reduceRight",
            "toReversed",
            "toSorted",
            "toSpliced",
            "with",
            "toLocaleString",
            "toString",
        ],
        &[symbol::ITERATOR, symbol::UNSCOPABLES],
    );
    order(
        b,
        b.ids.string_prototype,
        &[
            "length",
            "constructor",
            "anchor",
            "at",
            "big",
            "blink",
            "bold",
            "charAt",
            "charCodeAt",
            "codePointAt",
            "concat",
            "endsWith",
            "fontcolor",
            "fontsize",
            "fixed",
            "includes",
            "indexOf",
            "isWellFormed",
            "italics",
            "lastIndexOf",
            "link",
            "localeCompare",
            "match",
            "matchAll",
            "normalize",
            "padEnd",
            "padStart",
            "repeat",
            "replace",
            "replaceAll",
            "search",
            "slice",
            "small",
            "split",
            "strike",
            "sub",
            "substr",
            "substring",
            "sup",
            "startsWith",
            "toString",
            "toWellFormed",
            "trim",
            "trimStart",
            "trimLeft",
            "trimEnd",
            "trimRight",
            "toLocaleLowerCase",
            "toLocaleUpperCase",
            "toLowerCase",
            "toUpperCase",
            "valueOf",
        ],
        &[],
    );
    order(
        b,
        b.ids.number_prototype,
        &[
            "constructor",
            "toExponential",
            "toFixed",
            "toPrecision",
            "toString",
            "valueOf",
            "toLocaleString",
        ],
        &[],
    );
    order(
        b,
        b.ids.symbol_prototype,
        &[],
        &[symbol::TO_STRING_TAG, symbol::TO_PRIMITIVE],
    );
    order(
        b,
        b.ids.bigint_prototype,
        &["constructor", "toLocaleString", "toString", "valueOf"],
        &[],
    );
    order(
        b,
        b.ids.date_prototype,
        &[
            "constructor",
            "toString",
            "toDateString",
            "toTimeString",
            "toISOString",
            "toUTCString",
            "toGMTString",
            "getDate",
            "setDate",
            "getDay",
            "getFullYear",
            "setFullYear",
            "getHours",
            "setHours",
            "getMilliseconds",
            "setMilliseconds",
            "getMinutes",
            "setMinutes",
            "getMonth",
            "setMonth",
            "getSeconds",
            "setSeconds",
            "getTime",
            "setTime",
            "getTimezoneOffset",
            "getUTCDate",
            "setUTCDate",
            "getUTCDay",
            "getUTCFullYear",
            "setUTCFullYear",
            "getUTCHours",
            "setUTCHours",
            "getUTCMilliseconds",
            "setUTCMilliseconds",
            "getUTCMinutes",
            "setUTCMinutes",
            "getUTCMonth",
            "setUTCMonth",
            "getUTCSeconds",
            "setUTCSeconds",
            "valueOf",
            "getYear",
            "setYear",
            "toJSON",
            "toLocaleString",
            "toLocaleDateString",
            "toLocaleTimeString",
        ],
        &[],
    );
    order(
        b,
        b.ids.regexp_prototype,
        &[
            "constructor",
            "exec",
            "dotAll",
            "flags",
            "global",
            "hasIndices",
            "ignoreCase",
            "multiline",
            "source",
            "sticky",
            "unicode",
            "unicodeSets",
            "compile",
            "toString",
            "test",
        ],
        &[
            symbol::MATCH,
            symbol::MATCH_ALL,
            symbol::REPLACE,
            symbol::SEARCH,
            symbol::SPLIT,
        ],
    );
    order(
        b,
        b.ids.map_prototype,
        &[
            "constructor",
            "get",
            "set",
            "has",
            "delete",
            "clear",
            "entries",
            "forEach",
            "keys",
            "size",
            "values",
        ],
        &[symbol::TO_STRING_TAG, symbol::ITERATOR],
    );
    order(
        b,
        b.ids.set_prototype,
        &[
            "constructor",
            "has",
            "add",
            "delete",
            "difference",
            "clear",
            "entries",
            "forEach",
            "intersection",
            "isSubsetOf",
            "isSupersetOf",
            "isDisjointFrom",
            "size",
            "symmetricDifference",
            "union",
            "values",
            "keys",
        ],
        &[symbol::TO_STRING_TAG, symbol::ITERATOR],
    );
    order(
        b,
        b.ids.weak_map_prototype,
        &["constructor", "delete", "get", "set", "has"],
        &[],
    );
    order(
        b,
        b.ids.weak_set_prototype,
        &["constructor", "delete", "has", "add"],
        &[],
    );
    order(
        b,
        b.ids.promise_prototype,
        &["constructor", "then", "catch", "finally"],
        &[],
    );
    order(
        b,
        prototype(b, global(b, "DisposableStack")),
        &[
            "constructor",
            "use",
            "dispose",
            "adopt",
            "defer",
            "move",
            "disposed",
        ],
        &[],
    );
    order(
        b,
        prototype(b, global(b, "AsyncDisposableStack")),
        &[
            "constructor",
            "use",
            "disposeAsync",
            "adopt",
            "defer",
            "move",
            "disposed",
        ],
        &[],
    );
    order(
        b,
        prototype(b, global(b, "Iterator")),
        &[
            "constructor",
            "reduce",
            "toArray",
            "forEach",
            "some",
            "every",
            "find",
            "map",
            "filter",
            "take",
            "drop",
            "flatMap",
        ],
        &[],
    );
}

/** Math, JSON, Reflect, Atomics and WebAssembly. */
fn namespaces(b: &mut Builder) {
    order(
        b,
        global(b, "Math"),
        &[
            "abs", "acos", "acosh", "asin", "asinh", "atan", "atanh", "atan2", "ceil", "cbrt",
            "expm1", "clz32", "cos", "cosh", "exp", "floor", "f16round", "fround", "hypot", "imul",
            "log", "log1p", "log2", "log10", "max", "min", "pow", "random", "round", "sign", "sin",
            "sinh", "sqrt", "tan", "tanh", "trunc", "E", "LN10", "LN2", "LOG10E", "LOG2E", "PI",
            "SQRT1_2", "SQRT2",
        ],
        &[],
    );
    order(
        b,
        global(b, "JSON"),
        &["parse", "stringify", "rawJSON", "isRawJSON"],
        &[],
    );
    order(
        b,
        global(b, "Reflect"),
        &[
            "defineProperty",
            "deleteProperty",
            "apply",
            "construct",
            "get",
            "getOwnPropertyDescriptor",
            "getPrototypeOf",
            "has",
            "isExtensible",
            "ownKeys",
            "preventExtensions",
            "set",
            "setPrototypeOf",
        ],
        &[],
    );
    order(
        b,
        global(b, "Atomics"),
        &[
            "load",
            "store",
            "add",
            "sub",
            "and",
            "or",
            "xor",
            "exchange",
            "compareExchange",
            "isLockFree",
            "wait",
            "waitAsync",
            "notify",
            "pause",
        ],
        &[],
    );
    order(
        b,
        global(b, "WebAssembly"),
        &[
            "compile",
            "validate",
            "instantiate",
            "Module",
            "Instance",
            "Table",
            "Memory",
            "Global",
            "Tag",
            "JSTag",
            "Exception",
            "CompileError",
            "LinkError",
            "RuntimeError",
            "compileStreaming",
            "instantiateStreaming",
            "Suspending",
            "promising",
            "SuspendError",
        ],
        &[],
    );
}

/** Buffers, views and %TypedArray%. */
fn typed_arrays(b: &mut Builder) {
    order(
        b,
        b.ids.array_buffer_prototype,
        &[
            "constructor",
            "byteLength",
            "slice",
            "maxByteLength",
            "resizable",
            "resize",
            "transfer",
            "transferToFixedLength",
            "detached",
        ],
        &[],
    );
    order(
        b,
        b.ids.shared_array_buffer_prototype,
        &[
            "constructor",
            "byteLength",
            "slice",
            "maxByteLength",
            "growable",
            "grow",
        ],
        &[],
    );
    order(
        b,
        prototype(b, global(b, "DataView")),
        &[
            "constructor",
            "buffer",
            "byteLength",
            "byteOffset",
            "getInt8",
            "setInt8",
            "getUint8",
            "setUint8",
            "getInt16",
            "setInt16",
            "getUint16",
            "setUint16",
            "getInt32",
            "setInt32",
            "getUint32",
            "setUint32",
            "getFloat16",
            "setFloat16",
            "getFloat32",
            "setFloat32",
            "getFloat64",
            "setFloat64",
            "getBigInt64",
            "setBigInt64",
            "getBigUint64",
            "setBigUint64",
        ],
        &[],
    );
    order(
        b,
        prototype(b, global(b, "Uint8Array")),
        &[
            "constructor",
            "BYTES_PER_ELEMENT",
            "toBase64",
            "setFromBase64",
            "toHex",
            "setFromHex",
        ],
        &[],
    );
    let typed = parent(b, global(b, "Int8Array"));
    order(
        b,
        typed,
        &["length", "name", "prototype", "of", "from"],
        &[],
    );
    order(
        b,
        prototype(b, typed),
        &[
            "constructor",
            "buffer",
            "byteLength",
            "byteOffset",
            "length",
            "entries",
            "keys",
            "values",
            "at",
            "copyWithin",
            "every",
            "fill",
            "filter",
            "find",
            "findIndex",
            "findLast",
            "findLastIndex",
            "forEach",
            "includes",
            "indexOf",
            "join",
            "lastIndexOf",
            "map",
            "reverse",
            "reduce",
            "reduceRight",
            "set",
            "slice",
            "some",
            "sort",
            "subarray",
            "toReversed",
            "toSorted",
            "with",
            "toLocaleString",
            "toString",
        ],
        &[symbol::TO_STRING_TAG, symbol::ITERATOR],
    );
}

/** Generators. */
fn iteration(b: &mut Builder) {
    let generator = ["constructor", "next", "return", "throw"];
    order(b, b.ids.generator_prototype, &generator, &[]);
    order(b, b.ids.async_generator_prototype, &generator, &[]);
}

/** Intl and its service constructors' prototypes. */
fn intl(b: &mut Builder) {
    let intl = global(b, "Intl");
    order(
        b,
        intl,
        &[
            "getCanonicalLocales",
            "supportedValuesOf",
            "DateTimeFormat",
            "NumberFormat",
            "Collator",
            "PluralRules",
            "RelativeTimeFormat",
            "ListFormat",
            "Locale",
            "DisplayNames",
            "Segmenter",
            "DurationFormat",
        ],
        &[],
    );
    let formatter = [
        "constructor",
        "resolvedOptions",
        "formatToParts",
        "format",
        "formatRange",
        "formatRangeToParts",
    ];
    order(
        b,
        prototype(b, member(b, intl, "DateTimeFormat")),
        &formatter,
        &[],
    );
    order(
        b,
        prototype(b, member(b, intl, "NumberFormat")),
        &formatter,
        &[],
    );
    order(
        b,
        prototype(b, member(b, intl, "Collator")),
        &["constructor", "resolvedOptions", "compare"],
        &[],
    );
    order(
        b,
        prototype(b, member(b, intl, "PluralRules")),
        &["constructor", "resolvedOptions", "select", "selectRange"],
        &[],
    );
    let parts = ["constructor", "resolvedOptions", "format", "formatToParts"];
    order(
        b,
        prototype(b, member(b, intl, "RelativeTimeFormat")),
        &parts,
        &[],
    );
    order(b, prototype(b, member(b, intl, "ListFormat")), &parts, &[]);
    order(
        b,
        prototype(b, member(b, intl, "Locale")),
        &[
            "constructor",
            "toString",
            "maximize",
            "minimize",
            "language",
            "script",
            "region",
            "baseName",
            "calendar",
            "caseFirst",
            "collation",
            "firstDayOfWeek",
            "hourCycle",
            "numeric",
            "numberingSystem",
            "getCalendars",
            "getCollations",
            "getHourCycles",
            "getNumberingSystems",
            "getTimeZones",
            "getTextInfo",
            "getWeekInfo",
        ],
        &[],
    );
    order(
        b,
        prototype(b, member(b, intl, "DisplayNames")),
        &["constructor", "resolvedOptions", "of"],
        &[],
    );
    order(
        b,
        prototype(b, member(b, intl, "Segmenter")),
        &["constructor", "resolvedOptions", "segment"],
        &[],
    );
}

/** The ECMAScript names on the global object, in the order Chrome's window lists them; the
host's own interfaces, installed later, follow them. */
fn globals(b: &mut Builder) {
    const CHROME: [&str; 65] = [
        "Object",
        "Function",
        "Array",
        "Number",
        "parseFloat",
        "parseInt",
        "Infinity",
        "NaN",
        "undefined",
        "Boolean",
        "String",
        "Symbol",
        "Date",
        "Promise",
        "RegExp",
        "Error",
        "AggregateError",
        "EvalError",
        "RangeError",
        "ReferenceError",
        "SyntaxError",
        "TypeError",
        "URIError",
        "SuppressedError",
        "globalThis",
        "JSON",
        "Math",
        "Intl",
        "ArrayBuffer",
        "Atomics",
        "Uint8Array",
        "Int8Array",
        "Uint16Array",
        "Int16Array",
        "Uint32Array",
        "Int32Array",
        "BigUint64Array",
        "BigInt64Array",
        "Uint8ClampedArray",
        "Float32Array",
        "Float64Array",
        "Float16Array",
        "DataView",
        "Map",
        "BigInt",
        "Set",
        "Iterator",
        "WeakMap",
        "WeakSet",
        "Proxy",
        "Reflect",
        "FinalizationRegistry",
        "WeakRef",
        "DisposableStack",
        "AsyncDisposableStack",
        "decodeURI",
        "decodeURIComponent",
        "encodeURI",
        "encodeURIComponent",
        "escape",
        "unescape",
        "eval",
        "isFinite",
        "isNaN",
        "WebAssembly",
    ];
    let rank = |name: &str| {
        CHROME
            .iter()
            .position(|n| *n == name)
            .unwrap_or(CHROME.len())
    };
    b.globals.sort_by_key(|(name, _)| rank(name));
}

/** A global's intrinsic identity. */
fn global(b: &Builder, name: &str) -> ObjectId {
    b.globals
        .iter()
        .find_map(|(global, value)| (*global == name).then(|| value.object()))
        .flatten()
        .unwrap_or_else(|| panic!("global {name}"))
}

/** The object an intrinsic's own data property holds. */
fn member(b: &Builder, target: ObjectId, name: &str) -> ObjectId {
    let def = &b.objects[(target - INTRINSIC_BASE) as usize];
    match def.get(&Key::from(name)) {
        Some((_, Static::Value(JsValue::Object(id)), _)) => *id,
        _ => panic!("member {name}"),
    }
}

/** A constructor's `prototype`. */
fn prototype(b: &Builder, ctor: ObjectId) -> ObjectId {
    member(b, ctor, "prototype")
}

/** The intrinsic an intrinsic inherits from. */
fn parent(b: &Builder, target: ObjectId) -> ObjectId {
    b.objects[(target - INTRINSIC_BASE) as usize]
        .proto
        .expect("intrinsic parent")
}
