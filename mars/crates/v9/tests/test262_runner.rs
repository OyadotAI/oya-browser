/** Conformance scoreboard for V9: runs tc39's test262 and V8's own mjsunit through V9's public page
API (`load_page` to run, `check` for parse-phase negatives) and writes per-directory counts.
Ignored by default; run it with

    TEST262=built-ins/Array cargo test --release --offline -p mars-v9 --features conformance --test test262_runner -- --ignored --nocapture test262
    MJSUNIT=es6 cargo test --release --offline -p mars-v9 --features conformance --test test262_runner -- --ignored --nocapture mjsunit

`TEST262` / `MJSUNIT` are comma-separated path prefixes under `test/` (test262) or the mjsunit
folder; unset means everything. `TEST262_ROOT` / `MJSUNIT_ROOT` override the checkouts,
`CONF_OUT` the results folder, `CONF_JOBS` the thread count, `CONF_TIMEOUT_MS` the per-run timeout.
Set `CONF_REQUIRE_PASS=1` to fail the command after saving results when any executed case fails.
The opt-in `conformance` feature supplies d8.file fixture operations; the browser build omits it. */
mod common;
#[path = "support/mjsunit.rs"]
mod mjsunit_host;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/** Each case's result slot, filled by whichever worker ran it. */
type Outcomes = Arc<Mutex<Vec<Option<Result<(), String>>>>>;

/** Features V9 does not claim (its README: no modules, with, async generators, for await,
SharedArrayBuffer, Atomics; language support stops at ES2023; Intl and Temporal are claimed),
plus host hooks it lacks. `$262.detachArrayBuffer` is supplied through the engine's own
`ArrayBuffer.prototype.transfer`, which detaches its receiver, so detachment tests run, and
`$262.evalScript` is the engine's script evaluator when the `conformance` feature exposes it
(run test262 with the feature; without it the prelude falls back to indirect eval, which gives
eval rather than script declaration semantics). */
const UNCLAIMED: &[&str] = &[
    "SharedArrayBuffer",
    "Atomics",
    "Atomics.waitAsync",
    "Atomics.pause",
    "cross-realm",
    "IsHTMLDDA",
    "host-gc-required",
    "tail-call-optimization",
    "import-text",
    "import-assertions",
    "import-defer",
    "json-modules",
    "source-phase-imports",
    "source-phase-imports-module-source",
    "ShadowRealm",
    "decorators",
    "explicit-resource-management",
    "iterator-helpers",
    "iterator-sequencing",
    "joint-iteration",
    "set-methods",
    "array-grouping",
    "promise-with-resolvers",
    "promise-try",
    "resizable-arraybuffer",
    "arraybuffer-transfer",
    "immutable-arraybuffer",
    "regexp-v-flag",
    "regexp-modifiers",
    "regexp-duplicate-named-groups",
    "Float16Array",
    "Math.sumPrecise",
    "Error.isError",
    "upsert",
    "uint8array-base64",
    "json-parse-with-source",
    "String.prototype.isWellFormed",
    "String.prototype.toWellFormed",
    "await-dictionary",
    "canonical-tz",
    "legacy-regexp",
    "nonextensible-applies-to-private",
    "error-stack-accessor",
    "iterator-chunking",
];
/** mjsunit folders whose status file skips them on a V8 built without that JIT tier (the
`not has_maglev` block skips the whole maglev folder, and turbolev likewise): V9 has no tier. */
const MJSUNIT_JIT_DIRS: &[&str] = &["maglev", "turbolev"];
/** Tests whose status rows pass only in a build V9 does not resemble: the noi18n regexp
folding test is `[['not i18n', PASS], ['i18n', FAIL]]`, and V9 folds with ICU data. */
const MJSUNIT_UPSTREAM_FAIL: &[&str] = &["es6/unicode-regexp-ignore-case-noi18n"];
/** Whole test262 folders V9 does not claim. */
const UNCLAIMED_DIRS: &[&str] = &["staging"];
/** mjsunit folders that test d8, wasm or the sandbox rather than the language. */
const MJSUNIT_SKIP_DIRS: &[&str] = &["wasm", "shared-memory", "sandbox", "d8", "tools"];
/** Host definitions test262 expects (INTERPRETING.md): print for async tests and a small $262.
`createRealm` makes a fresh realm through the conformance build's d8 `Realm` (with cross-realm
access, so a test can reach the other realm's constructors) and returns that realm's $262. */
const PRELUDE: &str = "var print = function (m) { console.log(String(m)); };\n\
function __v9CreateRealm() { var i = Realm.createAllowCrossRealmAccess(); \
return { global: Realm.global(i), agent: {}, createRealm: __v9CreateRealm, \
detachArrayBuffer: function (b) { if (!b.detached) b.transfer(); }, \
evalScript: function (s) { return Realm.eval(i, s); } }; }\n\
var $262 = { global: globalThis, agent: {}, detachArrayBuffer: function (b) { if (!b.detached) b.transfer(); }, \
createRealm: typeof Realm === 'object' ? __v9CreateRealm : undefined, \
evalScript: typeof __v9EvalScript === 'function' ? __v9EvalScript : function (s) { return (0, eval)(s); } };\n";

/** One test: its file, the source modes to run and what it must do. */
struct Case {
    rel: String,
    group: String,
    /** Full sources per mode ("strict" / "sloppy"). */
    runs: Vec<(&'static str, String)>,
    negative: Option<(String, String)>,
    is_async: bool,
    /** The host loader for tests that load modules from sibling files on disk. */
    host: Option<Host>,
    /** Discovery failures remain scored failures rather than disappearing. */
    discovery_error: Option<String>,
}

/** test262's module host: the test file is the base URL, and module requests read files beside it. */
#[derive(Clone)]
struct Host {
    file: PathBuf,
    /** For `flags: [module]`: the harness, run as a classic script before the test module. */
    prelude: Option<String>,
    /** V8's separately parsed harness, declared prerequisites and actual test. */
    mjsunit: Option<mjsunit_host::Scripts>,
}

/** Frontmatter fields the runner reads. */
#[derive(Default)]
struct Meta {
    includes: Vec<String>,
    flags: Vec<String>,
    features: Vec<String>,
    negative: Option<(String, String)>,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap())
}

fn env_path(key: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(key).map(PathBuf::from).unwrap_or(default)
}

fn out_dir() -> PathBuf {
    env_path(
        "CONF_OUT",
        PathBuf::from(
            "/private/tmp/claude-501/-Users-mk-Dev-oya-agentchrome/0608a5c7-c382-49f9-b32d-bb2fdc1f40c0/scratchpad/conf/test262",
        ),
    )
}

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = mjsunit_host::confined::walk(dir).expect("confined conformance discovery failed");
    files.extend(entries.into_iter().map(|path| dir.join(path)));
}

/** Read a bounded fixture from a trusted root without following replaced path components. */
fn read_confined(root: &Path, relative: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let file = mjsunit_host::confined::open(root, relative)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "fixture is not a regular file",
        ));
    }
    let mut source = String::new();
    file.take((mars_v9::constants::MODULE_BYTES + 1) as u64)
        .read_to_string(&mut source)?;
    if source.len() > mars_v9::constants::MODULE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "fixture exceeds module byte limit",
        ));
    }
    Ok(source)
}

/** A YAML flow list `[a, b]` or the `- item` lines that follow the key. */
fn list(lines: &[&str], at: usize, rest: &str) -> Vec<String> {
    let rest = rest.trim();
    if let Some(inner) = rest.strip_prefix('[') {
        return inner
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    lines[at + 1..]
        .iter()
        .map(|l| l.trim())
        .take_while(|l| l.starts_with("- "))
        .map(|l| l[2..].trim().to_string())
        .collect()
}

fn meta(source: &str) -> Meta {
    let mut meta = Meta::default();
    let (Some(start), Some(end)) = (source.find("/*---"), source.find("---*/")) else {
        return meta;
    };
    let lines: Vec<&str> = source[start + 5..end].lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        match key {
            "includes" => meta.includes = list(&lines, i, rest),
            "flags" => meta.flags = list(&lines, i, rest),
            "features" => meta.features = list(&lines, i, rest),
            "negative" => {
                let field = |name: &str| {
                    lines[i + 1..]
                        .iter()
                        .take_while(|l| l.starts_with(' '))
                        .find_map(|l| l.trim().strip_prefix(name).map(|v| v.trim().to_string()))
                        .unwrap_or_default()
                };
                meta.negative = Some((field("phase:"), field("type:")));
            }
            _ => {}
        }
    }
    meta
}

/** Why a test262 file is skipped, or None to run it. */
fn skip262(rel: &str, meta: &Meta) -> Option<String> {
    let top = rel.split('/').next().unwrap_or("");
    if UNCLAIMED_DIRS.contains(&top) {
        return Some(format!("dir:{top}"));
    }
    if rel.contains("_FIXTURE") {
        return Some("fixture".into());
    }
    for flag in ["CanBlockIsTrue"] {
        if meta.flags.iter().any(|f| f == flag) {
            return Some(format!("flag:{flag}"));
        }
    }
    meta.features
        .iter()
        .find(|f| UNCLAIMED.contains(&f.as_str()))
        .map(|f| format!("feature:{f}"))
}

/** Directory bucket: the first two path parts (e.g. `built-ins/Array`). */
fn group(rel: &str, depth: usize) -> String {
    let parts: Vec<&str> = rel.split('/').collect();
    let n = depth.min(parts.len().saturating_sub(1)).max(1);
    if parts.len() == 1 {
        return "(root)".into();
    }
    parts[..n].join("/")
}

fn prefixes(key: &str) -> Vec<String> {
    std::env::var(key)
        .ok()
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn wanted(rel: &str, prefixes: &[String]) -> bool {
    prefixes.is_empty() || prefixes.iter().any(|p| rel.starts_with(p.as_str()))
}

/** Build the test262 cases plus the skip tally. */
fn cases262(root: &Path, skips: &mut BTreeMap<String, usize>) -> Vec<Case> {
    let harness = root.join("harness");
    let read = |name: &str| {
        read_confined(&harness, Path::new(name)).expect("confined Test262 harness fixture")
    };
    let base = format!("{PRELUDE}{}\n{}\n", read("assert.js"), read("sta.js"));
    let done = read("doneprintHandle.js");
    let filter = prefixes("TEST262");
    let mut files = Vec::new();
    walk(&root.join("test"), &mut files);
    files.sort();
    let mut cases = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(root.join("test"))
            .unwrap()
            .to_string_lossy()
            .to_string();
        if !wanted(&rel, &filter) {
            continue;
        }
        let source = match read_confined(&root.join("test"), Path::new(&rel)) {
            Ok(source) => source,
            Err(error) => {
                cases.push(Case {
                    group: group(&rel, 2),
                    rel,
                    runs: Vec::new(),
                    negative: None,
                    is_async: false,
                    host: None,
                    discovery_error: Some(format!("source read failed: {error}")),
                });
                continue;
            }
        };
        let meta = meta(&source);
        if let Some(reason) = skip262(&rel, &meta) {
            *skips.entry(reason).or_default() += 1;
            continue;
        }
        let has = |f: &str| meta.flags.iter().any(|x| x == f);
        let is_async = has("async");
        let module = has("module");
        let body = if has("raw") || module {
            source.clone()
        } else {
            let mut prelude = base.clone();
            if is_async {
                prelude.push_str(&done);
                prelude.push('\n');
            }
            for include in &meta.includes {
                prelude.push_str(&read(include));
                prelude.push('\n');
            }
            format!("{prelude}{source}")
        };
        let mut runs = Vec::new();
        let mut prelude = None;
        if module {
            let mut harness = base.clone();
            if is_async {
                harness.push_str(&done);
                harness.push('\n');
            }
            for include in &meta.includes {
                harness.push_str(&read(include));
                harness.push('\n');
            }
            prelude = Some(harness);
            runs.push(("module", body));
        } else if !has("onlyStrict") && !has("noStrict") && !has("raw") {
            runs.push(("sloppy", body.clone()));
            runs.push(("strict", format!("\"use strict\";\n{body}")));
        } else if has("onlyStrict") {
            runs.push(("strict", format!("\"use strict\";\n{body}")));
        } else {
            runs.push(("sloppy", body));
        }
        cases.push(Case {
            group: group(&rel, 2),
            rel,
            runs,
            negative: meta.negative,
            is_async,
            host: (module || source.contains("import")).then(|| Host {
                file: file.clone(),
                prelude,
                mjsunit: None,
            }),
            discovery_error: None,
        });
    }
    cases
}

/** Build the mjsunit cases: mjsunit.js as the harness, sloppy mode, pass means no uncaught error. */
fn cases_mjsunit(
    root: &Path,
    skips: &mut BTreeMap<String, usize>,
    exclusions: &mut Vec<Value>,
) -> Vec<Case> {
    let harness = read_confined(root, Path::new("mjsunit.js")).expect("confined mjsunit.js");
    let filter = prefixes("MJSUNIT");
    let admitted = include_mjsunit_dirs();
    let mut status_skips = unconditional_skips(root);
    let status_fails = unconditional_fails(root);
    status_skips.extend(simd_status_skips(root));
    status_skips.extend(non_linux_status_skips(root));
    let mut files = Vec::new();
    walk(root, &mut files);
    files.sort();
    let mut cases = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        if rel == "mjsunit.js" || !wanted(&rel, &filter) {
            continue;
        }
        let top = rel.split('/').next().unwrap_or("");
        if rel.contains('/')
            && MJSUNIT_SKIP_DIRS.contains(&top)
            && !admitted.contains(&top.to_string())
        {
            skip_case(skips, exclusions, &rel, format!("dir:{top}"));
            continue;
        }
        if rel.contains('/') && MJSUNIT_JIT_DIRS.contains(&top) {
            skip_case(skips, exclusions, &rel, format!("no-optimizer:dir:{top}"));
            continue;
        }
        if status_fails.iter().any(|pattern| status_matches(pattern, &rel))
            || MJSUNIT_UPSTREAM_FAIL.contains(&rel.trim_end_matches(".js"))
        {
            skip_case(skips, exclusions, &rel, "upstream:fail".into());
            continue;
        }
        let source = match read_confined(root, Path::new(&rel)) {
            Ok(source) => source,
            Err(error) => {
                cases.push(Case {
                    group: group(&rel, 1),
                    rel,
                    runs: Vec::new(),
                    negative: None,
                    is_async: false,
                    host: None,
                    discovery_error: Some(format!("source read failed: {error}")),
                });
                continue;
            }
        };
        let flags: String = source
            .lines()
            .filter(|l| l.starts_with("// Flags:"))
            .collect();
        let inert_native = std::env::var("MJSUNIT_INCLUDE_INERT_NATIVE_FLAG").as_deref() != Ok("0")
            && inert_native_syntax_flag(&source);
        let supported_native = std::env::var("MJSUNIT_INCLUDE_NATIVE_INTRINSICS").as_deref()
            != Ok("0")
            && supported_native_intrinsics(&source);
        if flags.contains("--allow-natives-syntax") && !inert_native && !supported_native {
            skip_case(
                skips,
                exclusions,
                &rel,
                "flag:--allow-natives-syntax".into(),
            );
            continue;
        }
        if status_skips
            .iter()
            .any(|pattern| status_matches(pattern, &rel))
            || (admitted.contains(&top.to_string()) && rel == "wasm/test-wasm-module-builder.js")
        {
            skip_case(skips, exclusions, &rel, "upstream:skip".into());
            continue;
        }
        if !supported_mjsunit_flags(&source.replace("--allow-natives-syntax", "")) {
            skip_case(
                skips,
                exclusions,
                &rel,
                format!("unsupported-flags:{flags}"),
            );
            continue;
        }
        if source.lines().any(|l| l.starts_with("// MODULE")) || rel.ends_with(".mjs") {
            skip_case(skips, exclusions, &rel, "module".into());
            continue;
        }
        cases.push(Case {
            group: group(&rel, 1),
            rel,
            runs: vec![("sloppy", source.clone())],
            negative: None,
            is_async: false,
            host: Some(Host {
                file: file.clone(),
                prelude: None,
                mjsunit: Some(mjsunit_host::Scripts::for_test(
                    root, &harness, &source, &file,
                )),
            }),
            discovery_error: None,
        });
    }
    cases
}

/** Respect V8's platform-specific exclusions only on non-Linux hosts. */
fn non_linux_status_skips(root: &Path) -> Vec<String> {
    let Ok(text) = read_confined(root, Path::new("mjsunit.status")) else { return Vec::new(); };
    non_linux_status_entries(&text, cfg!(target_os = "linux"))
}

/** Read the exact upstream non-Linux block rather than interpreting unrelated conditions. */
fn non_linux_status_entries(text: &str, linux: bool) -> Vec<String> {
    if linux { return Vec::new(); }
    let Some((_, tail)) = text.split_once("['system != linux', {") else { return Vec::new(); };
    let Some((block, _)) = tail.split_once("}],") else { return Vec::new(); };
    block.lines().filter_map(|line| {
        let line = line.trim();
        if !line.contains("[SKIP]") { return None; }
        line.strip_prefix('\'')?.split_once('\'').map(|(name, _)| name.to_string())
    }).collect()
}

/** V9 implements SIMD in software, so upstream's no_simd_hardware condition is false. */
fn simd_status_skips(root: &Path) -> Vec<String> {
    let Ok(text) = read_confined(root, Path::new("mjsunit.status")) else {
        return Vec::new();
    };
    simd_status_entries(&text)
}

/** Read only the exact known condition, never infer unrelated V8 status variables. */
fn simd_status_entries(text: &str) -> Vec<String> {
    let Some((_, tail)) = text.split_once("['no_simd_hardware == False', {") else {
        return Vec::new();
    };
    let Some((block, _)) = tail.split_once("}],  # no_simd_hardware == False") else {
        return Vec::new();
    };
    block
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.contains("[SKIP]") {
                return None;
            }
            line.strip_prefix('\'')?
                .split_once('\'')
                .map(|(name, _)| name.to_string())
        })
        .collect()
}

/** A native-syntax flag is inert only when no percent token can introduce an intrinsic. */
fn inert_native_syntax_flag(source: &str) -> bool {
    !source.contains('%')
        && source
            .lines()
            .filter_map(|line| line.strip_prefix("// Flags:"))
            .flat_map(str::split_whitespace)
            .any(|flag| flag == "--allow-natives-syntax")
}

/** Only tests whose every percent token names an implemented intrinsic are considered for opt-in execution. */
fn supported_native_intrinsics(source: &str) -> bool {
    if source.lines().filter_map(|line| line.strip_prefix("// Flags:")).flat_map(str::split_whitespace).any(|flag| flag == "--fuzzing")
        && (source.contains("%WasmStruct") || source.contains("%WasmArray")) {
        return false;
    }
    if !source
        .lines()
        .filter_map(|line| line.strip_prefix("// Flags:"))
        .flat_map(str::split_whitespace)
        .any(|flag| flag == "--allow-natives-syntax")
    {
        return false;
    }
    let mut found = false;
    for (at, _) in source.match_indices('%') {
        let tail = source[at + 1..].trim_start();
        let name_length = tail
            .bytes()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
            .count();
        let name = &tail[..name_length];
        if name.is_empty() || !tail[name_length..].trim_start().starts_with('(') {
            continue;
        }
        if ![
            "AbortJS",
            "PerformMicrotaskCheckpoint",
            "ArrayBufferDetach",
            "Is64Bit",
            "ToLength",
            "AtomicsSynchronizationPrimitiveNumWaitersForTesting",
            "IsJSReceiver",
            "IsArray",
            "ToBoolean",
            "Typeof",
            "ToName",
            "CreateIterResultObject",
            "GetWeakCollectionSize",
            "AtomicsNumWaitersForTesting",
            "AtomicsNumUnresolvedAsyncPromisesForTesting",
            "SetAllowAtomicsWait",
            "IsAtomicsWaitAllowed",
            "IsSharedString",
            "IsSameHeapObject",
            "SharedGC",
            "GetInitializerFunction",
            "ArrayBufferDetachForceWasm",
            "GetWasmExceptionTagId",
            "GetWasmExceptionValues",
            "ThrowStackOverflow",
            "StringMaxLength",
            "ConstructConsString",
            "ConstructSlicedString",
            "FlattenString",
            "InternalizeString",
            "ConstructInternalizedString",
            "ConstructThinString",
            "AllocateHeapNumber",
            "MaxSmi",
            "ArrayBufferMaxByteLength",
            "WasmStruct",
            "WasmArray",
            "IsWasmTrapHandlerEnabled",
            "StringLessThan",
            "StringLessThanOrEqual",
            "StringGreaterThan",
            "StringGreaterThanOrEqual",
            "StringEqual",
            "StringCompare",
            "Call",
            "ToNumber",
            "ToString",
            "ToNumeric",
            "ConstructDouble",
            "GetHoleNaN",
            "GetHoleNaNUpper",
            "GetHoleNaNLower",
            "HeapObjectVerify",
            "GetOptimizationStatus",
            "DebugPrint",
            "CompileBaseline",
            "IsBeingInterpreted",
            "ActiveTierIsIgnition",
            "IsMaglevEnabled",
            "IsTurbofanEnabled",
            "IsSparkplugEnabled",
            "IsConcurrentRecompilationSupported",
            "ActiveTierIsMaglev",
            "ActiveTierIsTurbofan",
            "ActiveTierIsSparkplug",
            "IsDictPropertyConstTrackingEnabled",
            "IsUndefinedDoubleEnabled",
            "PretenureAllocationSite",
            "CurrentFrameIsTurbofan",
            "PrepareFunctionForOptimization",
            "OptimizeFunctionOnNextCall",
            "OptimizeMaglevOnNextCall",
            "NeverOptimizeFunction",
            "OptimizeOsr",
            "BaselineOsr",
            "DeoptimizeFunction",
            "DeoptimizeNow",
            "_DeoptimizeNow",
            "ClearFunctionFeedback",
            "EnsureFeedbackVectorForFunction",
            "TurbofanStaticAssert",
            "AssertEscapeAnalysisElided",
            "AssertPeeled",
            "DisableOptimizationFinalization",
            "FinalizeOptimization",
            "WaitForBackgroundOptimization",
            "WasmTierUpFunction",
            "SetAllocationTimeout",
            "SimulateNewspaceFull",
            "DebugToggleBlockCoverage",
        ]
        .contains(&name)
        {
            return false;
        }
        found = true;
    }
    found
}

#[test]
/** Mixed or unknown private intrinsics stay excluded rather than faking unsupported state. */
fn native_admission_requires_only_implemented_intrinsics() {
    assert!(supported_native_intrinsics("// Flags: --allow-natives-syntax\n%AllocateHeapNumber();"));
    assert!(supported_native_intrinsics("// Flags: --allow-natives-syntax\n%MaxSmi();%ArrayBufferMaxByteLength();"));
    assert!(supported_native_intrinsics("// Flags: --allow-natives-syntax\n%WasmStruct();%WasmArray();"));
    assert!(supported_native_intrinsics("// Flags: --allow-natives-syntax\n%IsWasmTrapHandlerEnabled();"));
    assert!(!supported_native_intrinsics("// Flags: --allow-natives-syntax --fuzzing\n%WasmStruct();"));
    assert!(!supported_native_intrinsics("// Flags: --allow-natives-syntax\n%AllocateHeapNumber();%IsSmi(0);"));
    assert!(supported_native_intrinsics(
        "// Flags: --allow-natives-syntax\n%InternalizeString('value');"
    ));
    assert!(supported_native_intrinsics(
        "// Flags: --allow-natives-syntax\n%InternalizeString('value');%OptimizeFunctionOnNextCall(f);"
    ));
    assert!(!supported_native_intrinsics(
        "// Flags: --allow-natives-syntax\n%HasSmiElements(a);%OptimizeFunctionOnNextCall(f);"
    ));
    assert!(supported_native_intrinsics(
        "// Flags: --allow-natives-syntax\n%ArrayBufferDetach(a);%Is64Bit();"
    ));
    assert!(supported_native_intrinsics(
        "// Flags: --allow-natives-syntax\n%ArrayBufferDetach(a);%OptimizeFunctionOnNextCall(f);"
    ));
    assert!(!supported_native_intrinsics(
        "// Flags: --allow-natives-syntax\nlet n=5%2;"
    ));
    assert!(supported_native_intrinsics(
        "// Flags: --allow-natives-syntax\nlet n=5%2;%ToLength(n);"
    ));
    assert!(!supported_native_intrinsics("%ArrayBufferDetach(a);"));
}

#[test]
/** Percent tokens keep even potentially native syntax out of the inert-flag selection. */
fn inert_native_flag_requires_no_percent_token() {
    assert!(inert_native_syntax_flag(
        "// Flags: --allow-natives-syntax\nlet x = 1;"
    ));
    assert!(!inert_native_syntax_flag(
        "// Flags: --allow-natives-syntax\n%OptimizeFunctionOnNextCall(f);"
    ));
    assert!(!inert_native_syntax_flag(
        "// Flags: --allow-natives-syntax\nlet x = 4 % 2;"
    ));
    assert!(!inert_native_syntax_flag("let x = 1;"));
}

/** Admit only flag combinations actually implemented by the conformance host. */
fn supported_mjsunit_flags(source: &str) -> bool {
    for line in source
        .lines()
        .filter_map(|line| line.strip_prefix("// Flags:"))
    {
        let mut flags = line.split_whitespace();
        while let Some(flag) = flags.next() {
            if flag == "--" {
                break;
            }
            if matches!(
                flag,
                "--bundle"
                    | "--expose-externalize-string"
                    | "--harmony-struct"
                    | "--expose-gc"
                    | "--expose-async-hooks"
                    | "--async-stack-traces"
                    | "--noasync-stack-traces"
                    | "--no-async-stack-traces"
                    | "--ignore-unhandled-promises"
                    | "--throws"
                    | "--harmony-shadow-realm"
                    | "--js-sum-precise"
                    | "--js-iterator-includes"
                    | "--js-iterator-join"
                    | "--enable-queue-microtask"
                    | "--noincremental-marking"
                    | "--no-incremental-marking"
                    | "--shared-string-table"
                    | "--wasm-test-streaming"
                    | "--wasm-stringref"
                    | "--wasm-custom-descriptors"
                    | "--wasm-acquire-release"
                    | "--wasm-imported-strings-utf8"
                    | "--wasm-fp16"
                    | "--wasm-js-interop"
                    | "--wasm-wide-arithmetic"
                    | "--wasm-wasmfx"
                    | "--wasm-shared"
                    | "--wasm-compact-imports"
                    | "--wasm-ref-cast-nop"
                    | "--wasm-math-intrinsics"
                    | "--no-wasm-math-intrinsics"
                    | "--enable-testing-opcode-in-wasm"
                    | "--no-wasm-lazy-compilation"
                    | "--no-maglev"
                    | "--no-turbofan"
                    | "--nowasm-lazy-compilation"
                    | "--no-wasm-inlining"
                    | "--no-enable-avx"
                    | "--no-enable-sse4-1"
                    | "--no-enable-sse4-2"
                    | "--no-enable-ssse3"
                    | "--no-wasm-trap-handler"
                    | "--wasm-enforce-bounds-checks"
                    | "--no-wasm-loop-unrolling"
                    | "--no-wasm-loop-peeling"
                    | "--no-wasm-opt"
                    | "--no-wasm-dynamic-tiering"
                    | "--no-wasm-tier-up"
                    | "--wasm-num-compilation-tasks=0"
                    | "--no-wasm-native-module-cache"
                    | "--wasm-allow-mixed-eh-for-testing"
            ) {
                continue;
            }
            if inert_d8_flag(flag) {
                continue;
            }
            if flag == "--wasm-num-compilation-tasks" {
                if flags.next() != Some("0") { return false; }
                continue;
            }
            let numeric = ["--wasm-max-module-size", "--wasm-max-mem-pages", "--wasm-max-table-size"];
            if numeric.contains(&flag) {
                if !flags
                    .next()
                    .is_some_and(|value| value.parse::<usize>().is_ok())
                {
                    return false;
                }
                continue;
            }
            if numeric.iter().any(|name| {
                flag.strip_prefix(name)
                    .and_then(|suffix| suffix.strip_prefix('='))
                    .is_some_and(|value| value.parse::<usize>().is_ok())
            }) {
                continue;
            }
            return false;
        }
    }
    true
}

/** d8 flags that tune V8 machinery V9 does not have: its JIT tiers and their budgets, its
generational collector, its lazy parser and background compilers, and its diagnostic printing. Wasm compiler flags stay out: lazy compilation moves validation errors to
first call, so they do change what a program means. A test carrying one of these asks the same JavaScript question of V9 as of
V8, so refusing it hid language coverage for nothing. Flags that change what a program means
(staging features, ICU switches, the inspector, code caching, strictness) are not here. */
const INERT_D8_FLAGS: &[&str] = &[
    // JIT tiers, budgets and feedback.
    "--no-lazy-feedback-allocation", "--lazy-feedback-allocation", "--invocation-count-for-turbofan",
    "--invocation-count-for-maglev", "--invocation-count-for-maglev-osr", "--interrupt-budget",
    "--interrupt-budget-for-feedback-allocation", "--jit-fuzzing", "--no-jit-fuzzing", "--maglev",
    "--no-maglev", "--turbofan", "--no-turbofan", "--noturbofan", "--always-turbofan", "--turbolev",
    "--turbolev-future", "--turboshaft", "--no-turbo", "--turbo-filter", "--always-osr", "--use-osr",
    "--ignition-osr", "--osr-to-tierup", "--no-use-ic", "--nouse-ic", "--deopt-every-n-times",
    "--no-concurrent-recompilation", "--no-concurrent-osr", "--no-concurrent-sparkplug",
    "--stress-concurrent-inlining", "--assert-types", "--turbo-loop-variable", "--turbo-loop-rotation",
    "--turbo-stats-nvp", "--noturbo-inlining", "--no-turbo-escape", "--no-turbo-allocation-folding",
    "--turboshaft-assert-types", "--turboshaft-typed-optimizations", "--maglev-non-eager-inlining",
    "--max-maglev-inlined-bytecode-size-small", "--maglev-untagged-phis", "--no-maglev-untagged-phis",
    "--maglev-extend-properties-backing-store", "--minimum-invocations-after-ic-update",
    "--minimum-invocations-before-optimization", "--function-context-specialization",
    "--noanalyze-environment-liveness", "--no-analyze-environment-liveness", "--jitless",
    "--efficiency-mode", "--for-of-optimization", "--force-slow-path", "--no-force-slow-path",
    "--no-fast-math", "--no-inline-new", "--noinline-new", "--omit-default-ctors",
    "--proto-assign-seq-opt", "--superspreading", "--cache-property-key-string-adds",
    "--typed-array-length-loading", "--use-std-math-pow", "--test-small-max-function-context-stub-size",
    "--stress-lazy-compilation", "--nocompilation-cache", "--random-seed", "--flag-processing-mode",
    "--predictable", "--single-threaded", "--stress-runs", "--stress-delay-tasks",
    "--no-wait-for-background-tasks", "--future",
    // The regexp tiers and the experimental backtracking engine.
    "--no-regexp-tier-up", "--regexp-tier-up", "--noregexp-optimization",
    "--enable-experimental-regexp-engine", "--no-enable-experimental-regexp-engine",
    "--enable-experimental-regexp-engine-on-excessive-backtracks", "--regexp-backtracks-before-fallback",
    "--experimental-regexp-engine-capture-group-opt",
    "--experimental-regexp-engine-capture-group-opt-max-memory-usage",
    "--no-default-to-experimental-regexp-engine", "--no-regexp-interpret-all",
    // The generational collector and its stress modes.
    "--gc-global", "--no-gc-global", "--gc-interval", "--max-semi-space-size", "--min-semi-space-size",
    "--max-old-space-size", "--stress-compaction", "--no-stress-compaction", "--verify-heap",
    "--noverify-heap", "--invoke-weak-callbacks", "--minor-ms", "--no-minor-ms", "--stress-scavenge",
    "--no-single-generation", "--handle-weak-ref-weakly-in-minor-gc", "--no-scavenger-chaos-mode",
    "--stress-marking", "--trace-gc-object-stats", "--trace-evacuation", "--stress-flush-code",
    "--no-stress-flush-code", "--flush-bytecode", "--stress-flush-bytecode", "--multi-mapped-mock-allocator",
    "--check-handle-count", "--stress-per-context-marking-worklist",
    "--stress-scavenger-conservative-object-pinning-random", "--track-array-buffer-views", "--code-stats",
    "--empty-shared-heap", "--nostress-incremental-marking", "--no-stress-incremental-marking",
    "--no-stress-concurrent-allocation", "--track-field-types", "--typed-array-max-size-in-heap",
    "--max-lazy", "--stress-wasm-code-gc", "--stress-snapshot",
    // Printing, tracing and logging that only produce output.
    "--print-code", "--print-ast", "--print-wasm-code", "--code-comments", "--trace-ignition",
    "--trace-turbo-graph", "--trace-turbo-reduction", "--trace-wasm", "--trace-wasm-decoder",
    "--trace-wasm-memory", "--trace-regexp-bytecodes", "--trace-generalization", "--trace-migration",
    "--trace-normalization", "--log-code", "--log", "--logfile", "--enable-tracing", "--dump-counters",
    "--perf-prof", "--perf-prof-delete-file", "--perf-prof-unwinding-info", "--print-opt-code-filter",
    "--print-builtin-code-filter", "--debug-code", "--enable-slow-asserts",
    "--interpreted-frames-native-stack", "--cpu-profiler-sampling-interval",
    // Lazy parsing and background compilation.
    "--lazy", "--no-lazy", "--nolazy", "--lazy-inner-functions", "--enable-lazy-source-positions",
    "--stress-lazy-source-positions", "--stress-lazy", "--preparser-scope-analysis",
    "--parallel-compile-tasks-for-lazy", "--parallel-compile-tasks-for-eager-toplevel",
    "--compiler-dispatcher", "--stress-background-compile", "--no-stress-background-compile",
    "--streaming-compile", "--no-script-streaming", "--compile-hints-magic", "--array-destructure-bytecode",
    "--use-external-strings", "--string-slices",
    // Native stack sizing; V9 bounds frames by count, not bytes.
    "--stack-size", "--sim-stack-size",
];

/** A flag is inert when its name, before any `=value`, tunes machinery V9 does not have. */
fn inert_d8_flag(flag: &str) -> bool {
    let name = flag.split_once('=').map_or(flag, |(name, _)| name);
    INERT_D8_FLAGS.contains(&name)
}

#[test]
/** Unconditional FAIL rows are read like SKIP rows, and mixed rows stay out of both. */
fn status_fail_rows_are_parsed_separately_from_skips() {
    let text = "[ALWAYS, {\n 'bugs/*': [FAIL],\n 'flaky': [PASS, FAIL],\n 'gone': [SKIP],\n}],  # ALWAYS";
    assert_eq!(always_status_entries(text, "[FAIL]"), vec!["bugs/*".to_string()]);
    assert_eq!(always_status_entries(text, "[SKIP]"), vec!["gone".to_string()]);
}

#[test]
/** Unsupported flags in any line or combination cannot be admitted as no-ops. */
fn mjsunit_admission_requires_all_flags_to_be_implemented() {
    // V9 has neither optimizer, so asking for one changes nothing about its execution state.
    assert!(supported_mjsunit_flags("// Flags: --no-maglev --no-turbofan\n"));
    assert!(supported_mjsunit_flags("// Flags: --no-maglev --turbofan\n"));
    assert!(supported_mjsunit_flags("// Flags: --stack-size=100 --interrupt-budget=1024\n"));
    // --no-fail tells d8 to ignore failures, which is not a tuning flag.
    assert!(!supported_mjsunit_flags("// Flags: --no-fail\n"));
    assert!(!supported_mjsunit_flags("// Flags: --stack-size=100 --js-staging\n"));
    assert!(!supported_mjsunit_flags("// Flags: --enable-inspector\n"));
    assert!(supported_mjsunit_flags("// Flags: --js-iterator-includes --js-iterator-join\n"));
    assert!(!supported_mjsunit_flags("// Flags: --js-iterator-includes --unknown\n"));
    assert!(supported_mjsunit_flags("// Flags: --async-stack-traces --no-async-stack-traces\n"));
    assert!(!supported_mjsunit_flags("// Flags: --async-stack-traces --unknown\n"));
    assert!(supported_mjsunit_flags("// Flags: -- 1 --literal\n"));
    assert!(!supported_mjsunit_flags("// Flags: --unknown -- 1\n"));
    assert!(supported_mjsunit_flags("// Flags: --no-wasm-trap-handler --wasm-enforce-bounds-checks\n"));
    assert!(!supported_mjsunit_flags("// Flags: --no-wasm-trap-handler --unknown\n"));
    assert!(supported_mjsunit_flags(
        "// Flags: --expose-gc --harmony-struct\n// Flags: --wasm-max-module-size=128\n"
    ));
    assert!(supported_mjsunit_flags(
        "// Flags: --bundle --wasm-max-module-size 128\n"
    ));
    assert!(supported_mjsunit_flags(
        "// Flags: --no-incremental-marking\n"
    ));
    assert!(supported_mjsunit_flags(
        "// Flags: --expose-async-hooks --ignore-unhandled-promises --throws\n"
    ));
    assert!(supported_mjsunit_flags("// Flags: --shared-string-table\n"));
    assert!(supported_mjsunit_flags("// Flags: --js-sum-precise\n"));
    assert!(supported_mjsunit_flags("// Flags: --expose-externalize-string\n"));
    assert!(!supported_mjsunit_flags("// Flags: --expose-externalize-string --unknown\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-test-streaming\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-stringref\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-custom-descriptors\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-acquire-release\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-imported-strings-utf8\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-fp16\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-js-interop\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-wide-arithmetic\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-wasmfx\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-shared\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-compact-imports\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-ref-cast-nop\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-math-intrinsics --no-wasm-math-intrinsics\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-math-intrinsics --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-ref-cast-nop --unknown\n"));
    assert!(supported_mjsunit_flags("// Flags: --enable-testing-opcode-in-wasm\n"));
    assert!(!supported_mjsunit_flags("// Flags: --enable-testing-opcode-in-wasm --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-compact-imports --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-shared --unknown\n"));
    assert!(supported_mjsunit_flags("// Flags: --no-wasm-lazy-compilation --nowasm-lazy-compilation\n"));
    assert!(supported_mjsunit_flags("// Flags: --no-wasm-inlining --no-wasm-loop-unrolling --no-wasm-loop-peeling --no-wasm-opt --no-wasm-dynamic-tiering --no-wasm-native-module-cache\n"));
    assert!(!supported_mjsunit_flags("// Flags: --no-wasm-inlining --unknown\n"));
    assert!(supported_mjsunit_flags("// Flags: --no-enable-avx --no-enable-sse4-1 --no-enable-sse4-2 --no-enable-ssse3\n"));
    assert!(!supported_mjsunit_flags("// Flags: --no-enable-avx --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --enable-avx\n"));
    assert!(supported_mjsunit_flags("// Flags: --no-wasm-tier-up --wasm-num-compilation-tasks=0\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-num-compilation-tasks 0\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-num-compilation-tasks=1\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-lazy-compilation\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-wasmfx --unknown\n"));
    assert!(supported_mjsunit_flags("// Flags: --wasm-allow-mixed-eh-for-testing\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-allow-mixed-eh-for-testing --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-wide-arithmetic --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-js-interop --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-fp16 --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-imported-strings-utf8 --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-acquire-release --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-custom-descriptors --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-stringref --unknown\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-test-streaming --unknown\n"));
    assert!(!supported_mjsunit_flags(
        "// Flags: --shared-string-table --unknown\n"
    ));
    assert!(!supported_mjsunit_flags(
        "// Flags: --expose-gc --unknown\n"
    ));
    assert!(!supported_mjsunit_flags(
        "// Flags: --wasm-max-module-size=bad\n"
    ));
    assert!(!supported_mjsunit_flags(
        "// Flags: --wasm-max-module-size\n"
    ));
    assert!(supported_mjsunit_flags(
        "// Flags: --wasm-max-module-size=128 --wasm-shared\n"
    ));
    assert!(supported_mjsunit_flags(
        "// Flags: --wasm-max-table-size=10\n"
    ));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-max-table-size=bad\n"));
    assert!(!supported_mjsunit_flags("// Flags: --wasm-max-table-size=10 --unknown\n"));
}

/** Explicit admission does not change the default discovery profile. */
fn include_mjsunit_dirs() -> Vec<String> {
    let requested = prefixes("MJSUNIT_INCLUDE_DIRS");
    if !requested.is_empty() {
        assert!(
            cfg!(feature = "conformance"),
            "MJSUNIT_INCLUDE_DIRS requires --features conformance"
        );
    }
    for name in &requested {
        assert!(
            MJSUNIT_SKIP_DIRS.contains(&name.as_str()),
            "Unknown MJSUNIT_INCLUDE_DIRS entry {name}; choose wasm, shared-memory, d8, sandbox, or tools."
        );
    }
    requested
}

/** Count and retain each excluded path so a report can be audited or replayed. */
fn skip_case(
    skips: &mut BTreeMap<String, usize>,
    exclusions: &mut Vec<Value>,
    path: &str,
    reason: String,
) {
    *skips.entry(reason.clone()).or_default() += 1;
    exclusions.push(json!({"test":path,"reason":reason}));
}

/** Honor the pinned status file's unconditional SKIP entries for every discovery profile. */
fn unconditional_skips(root: &Path) -> Vec<String> {
    let text = read_confined(root, Path::new("mjsunit.status"))
        .expect("mjsunit discovery requires its upstream status file");
    always_status_entries(&text, "[SKIP]")
}

/** The status file's unconditional `[FAIL]` rows (the bugs folder): V8 fails these itself, so
they measure nothing about V9 and are reported as `upstream:fail`, never as V9 failures. */
fn unconditional_fails(root: &Path) -> Vec<String> {
    let text = read_confined(root, Path::new("mjsunit.status"))
        .expect("mjsunit discovery requires its upstream status file");
    always_status_entries(&text, "[FAIL]")
}

/** Parse only unconditional entries with exactly `outcome`; conditional rows inside the block
and mixed rows such as `[PASS, FAIL]` remain unassumed. */
fn always_status_entries(text: &str, outcome: &str) -> Vec<String> {
    let block = text
        .split_once("[ALWAYS, {")
        .expect("mjsunit ALWAYS status block")
        .1
        .split_once("}],  # ALWAYS")
        .expect("mjsunit ALWAYS status end")
        .0;
    block
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.contains(outcome) {
                return None;
            }
            line.strip_prefix('\'')?
                .split_once('\'')
                .map(|(name, _)| name.to_string())
        })
        .collect()
}

/** Status paths omit .js and use a final star for prefix patterns. */
fn status_matches(pattern: &str, path: &str) -> bool {
    let name = path.strip_suffix(".js").unwrap_or(path);
    pattern
        .strip_suffix('*')
        .map_or(name == pattern, |prefix| name.starts_with(prefix))
}

/** Run one source; Ok(()) on pass, Err(reason) on fail. */
fn run_one(
    source: &str,
    negative: &Option<(String, String)>,
    is_async: bool,
    host: Option<&Host>,
) -> Result<(), String> {
    if let Some(host) = host {
        return run_hosted(source, negative, is_async, host);
    }
    if let Some((phase, kind)) = negative
        && (phase == "parse" || phase == "early")
    {
        return match mars_v9::check(source) {
            Err(e)
                if kind != "SyntaxError"
                    || e.code == "script_syntax_error"
                    || e.message.contains("SyntaxError") =>
            {
                Ok(())
            }
            Err(e) => Err(format!("wrong parse error {kind}: {}", e.message)),
            Ok(()) => Err(format!("expected {phase} {kind}, parsed fine")),
        };
    }
    let host = common::host();
    let environment = common::environment();
    let sources = [source.to_string()];
    let execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).map_err(|e| e.message)?;
    let failure = execution
        .failed
        .first()
        .map(|f| (f.error.code.clone(), f.error.message.clone()));
    if let Some((_, kind)) = negative {
        return match failure {
            Some((_, m)) if m.contains(&format!("Uncaught {kind}")) => Ok(()),
            Some((_, m)) => Err(format!("expected runtime {kind}: {m}")),
            None => Err(format!("expected runtime {kind}, ran fine")),
        };
    }
    if let Some((code, message)) = failure {
        return Err(format!("{code}: {message}"));
    }
    if is_async {
        let console = common::console(&execution);
        if !console.iter().any(|l| l == "Test262:AsyncTestComplete") {
            let detail = console.iter().find(|l| l.starts_with("Test262:")).cloned();
            return Err(detail.unwrap_or_else(|| "async test never called $DONE".into()));
        }
    }
    Ok(())
}

/** Run a test through the module host: classic or module, serving each round of module
requests from the files beside the test, as the app serves fetches. */
fn run_hosted(
    source: &str,
    negative: &Option<(String, String)>,
    is_async: bool,
    host: &Host,
) -> Result<(), String> {
    if let Some(scripts) = &host.mjsunit {
        return scripts.execute();
    }
    use mars_v9::{EntryKind, ModuleEntry, ModuleInput, ModuleSource};
    let url = url::Url::from_file_path(&host.file).map_err(|_| "bad test path".to_string())?;
    let mut environment = common::environment();
    environment.url = url.to_string();
    let mut input = ModuleInput::default();
    let sources: Vec<String> = match &host.prelude {
        Some(prelude) => {
            input.entries.push(ModuleEntry {
                index: 1,
                kind: EntryKind::Module,
                url: Some(url.to_string()),
            });
            vec![prelude.clone(), String::new()]
        }
        None => vec![source.to_string()],
    };
    let page = common::host();
    let mut execution = mars_v9::load_page(mars_v9::PageInput {
        modules: &input,
        ..common::input(&sources, &page, &environment)
    })
    .map_err(|e| e.message)?;
    let mut failures = Vec::new();
    let mut console = Vec::new();
    for _ in 0..64 {
        failures.extend(
            execution
                .failed
                .iter()
                .map(|f| (f.error.code.clone(), f.error.message.clone())),
        );
        console.extend(common::console(&execution));
        if execution.module_requests.is_empty() {
            break;
        }
        for request in &execution.module_requests {
            let text = if request.url == url.as_str() && host.prelude.is_some() {
                Some(source.to_string())
            } else {
                url::Url::parse(&request.url)
                    .ok()
                    .and_then(|u| u.to_file_path().ok())
                    .and_then(|path| {
                        path.strip_prefix(test262_root())
                            .ok()
                            .and_then(|relative| read_confined(&test262_root(), relative).ok())
                    })
            };
            input.sources.push(ModuleSource {
                url: request.url.clone(),
                source: text,
                classic: false,
            });
        }
        execution = mars_v9::load_modules(
            mars_v9::PageInput {
                modules: &input,
                ..common::input(&sources, &page, &environment)
            },
            execution.state.clone(),
        )
        .map_err(|e| e.message)?;
    }
    // A module that rejects after a fetch round (top-level await) reports through the console.
    if host.prelude.is_some() {
        failures.extend(
            console
                .iter()
                .filter(|l| l.starts_with("Uncaught ") && !l.starts_with("Uncaught (in promise)"))
                .map(|l| ("module_rejected".to_string(), l.clone())),
        );
    }
    let failure = failures.into_iter().next();
    if let Some((_, kind)) = negative {
        return match failure {
            Some((code, m))
                if m.contains(kind.as_str())
                    || (kind == "SyntaxError" && code.contains("syntax")) =>
            {
                Ok(())
            }
            Some((_, m)) => Err(format!("expected {kind}: {m}")),
            None => Err(format!("expected {kind}, ran fine")),
        };
    }
    if let Some((code, message)) = failure {
        return Err(format!("{code}: {message}"));
    }
    if is_async && !console.iter().any(|l| l == "Test262:AsyncTestComplete") {
        let detail = console.iter().find(|l| l.starts_with("Test262:")).cloned();
        return Err(detail.unwrap_or_else(|| "async test never called $DONE".into()));
    }
    Ok(())
}

/** Run with a hard timeout; a hung or panicking run counts as a failure. */
fn run_guarded(
    source: String,
    negative: Option<(String, String)>,
    is_async: bool,
    host: Option<Host>,
    timeout: Duration,
) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    // ponytail: a timed-out worker thread is abandoned, not killed; V9's own script deadline ends it.
    thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let _ = tx.send(run_one(&source, &negative, is_async, host.as_ref()));
        })
        .map_err(|e| e.to_string())?;
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err("timeout".into()),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("panic".into()),
    }
}

/** A failure message with locations and numbers stripped, to bucket similar failures. */
fn bucket(message: &str) -> String {
    let message = match message.find("Script execution failed at line") {
        Some(at) => format!(
            "{}{}",
            &message[..at],
            message[at..].split_once(": ").map_or("", |p| p.1)
        ),
        None => message.to_string(),
    };
    let mut out = String::new();
    let mut digits = false;
    for c in message.chars().take(110) {
        if c.is_ascii_digit() {
            if !digits {
                out.push('N');
            }
            digits = true;
        } else {
            digits = false;
            out.push(c);
        }
    }
    out
}

/** Run every mode of one case; the first failing mode is the reason. */
fn run_case(case: &Case, timeout: Duration) -> Result<(), String> {
    if let Some(error) = &case.discovery_error {
        return Err(error.clone());
    }
    for (mode, source) in &case.runs {
        run_guarded(
            source.clone(),
            case.negative.clone(),
            case.is_async,
            case.host.clone(),
            timeout,
        )
        .map_err(|e| format!("[{mode}] {e}"))?;
    }
    Ok(())
}

/** Bind each worker request to the path, sources and fixtures discovered by the parent. */
fn case_identity(case: &Case) -> String {
    let fixtures = case
        .host
        .as_ref()
        .and_then(|host| host.mjsunit.as_ref())
        .map(|scripts| scripts.identity())
        .unwrap_or_default();
    let text = format!(
        "{:?}{:?}{:?}{:?}",
        case.rel, case.runs, case.discovery_error, fixtures
    );
    ring::digest::digest(&ring::digest::SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/** A changed or reordered corpus must never be scored as the parent's original test. */
fn request_matches(case: &Case, request: &Value) -> bool {
    request["path"] == case.rel && request["identity"] == case_identity(case)
}

/** Marks the worker's reply lines among libtest's own output. */
const REPLY: &str = "@@conf ";

/** A worker process: this same test binary running `conf_worker`, so an engine abort (V9 can
die on a huge allocation) loses one test, not the run. */
struct Worker {
    child: Child,
    replies: mpsc::Receiver<Option<Value>>,
    reader: thread::JoinHandle<()>,
}

/** A worker response has a bounded outer wait beyond its own case deadline. */
const WORKER_REPLY_GRACE: Duration = Duration::from_secs(5);
/** How long a fresh worker may take to rediscover the corpus before its first case. All of
test262 is 48k files to read and hash, longer than one case's deadline, and without this wait
the parent killed every worker mid-discovery and the run never advanced. */
const WORKER_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(600);
/** The worker's first reply, sent once its discovery matches the parent's and cases can be asked. */
const WORKER_READY: &str = "ready";
/** Limit child processes even when an environment variable is malformed or excessive. */
const MAX_WORKERS: usize = 32;
/** No single upstream case can hold a worker indefinitely. */
const MAX_CASE_TIMEOUT_MS: u64 = 300_000;

fn spawn_worker(suite: &str, binary: &Path) -> Worker {
    let mut child = Command::new(binary)
        .args([
            "conf_worker",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CONF_SUITE", suite)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("worker spawns");
    let mut output = child.stdout.take().expect("worker stdout");
    let (send, replies) = mpsc::channel();
    let reader = thread::spawn(move || {
        loop {
            let reply = read_reply(&mut output);
            let ended = reply.is_none();
            if send.send(reply).is_err() || ended {
                break;
            }
        }
    });
    // Discovery runs before the first case's clock starts: wait for the worker's ready reply.
    let ready = replies.recv_timeout(WORKER_DISCOVERY_TIMEOUT);
    if !matches!(ready, Ok(Some(Value::String(ref word))) if word == WORKER_READY) {
        eprintln!("{suite}: a worker did not finish discovery in time; its cases will fail");
    }
    Worker {
        child,
        replies,
        reader,
    }
}

/** Reap a worker before joining its pipe reader, including after timeouts. */
fn retire(worker: &mut Option<Worker>) {
    if let Some(mut worker) = worker.take() {
        let _ = worker.child.kill();
        let _ = worker.child.wait();
        let _ = worker.reader.join();
    }
}

/** Send case `i` to the worker (spawning one if needed) and read its verdict. */
fn ask(child: &mut Option<Worker>, suite: &str, binary: &Path, i: usize, case: &Case) -> Result<(), String> {
    let worker = child.get_or_insert_with(|| spawn_worker(suite, binary));
    let request = json!({"index":i,"path":case.rel,"identity":case_identity(case)});
    let reply = writeln!(worker.child.stdin.as_mut().unwrap(), "{request}")
        .ok()
        .and_then(|_| {
            worker
                .replies
                .recv_timeout(timeout() + WORKER_REPLY_GRACE)
                .ok()
                .flatten()
        });
    match reply {
        Some(Value::String(e)) => {
            if e.contains("timeout") {
                retire(child);
            }
            Err(e)
        }
        Some(_) => Ok(()),
        None => {
            retire(child);
            Err("worker closed or exceeded its response deadline".into())
        }
    }
}

/** Read lines until a reply: null for a pass, a string for a failure; None when the worker died. */
fn read_reply(stdout: &mut ChildStdout) -> Option<Value> {
    // ponytail: one byte-at-a-time reader so no buffered bytes are lost between calls.
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        line.clear();
        loop {
            match std::io::Read::read(stdout, &mut byte) {
                Ok(1) if byte[0] == b'\n' => break,
                Ok(1) => line.push(byte[0]),
                _ => return None,
            }
        }
        let text = String::from_utf8_lossy(&line);
        // libtest's "test conf_worker ... " shares the first reply's line.
        if let Some(at) = text.find(REPLY) {
            return serde_json::from_str(&text[at + REPLY.len()..]).ok();
        }
    }
}

/** Worker mode: rebuild the suite's cases, then answer case indices from stdin. */
#[test]
#[ignore]
fn conf_worker() {
    let Ok(suite) = std::env::var("CONF_SUITE") else {
        return;
    };
    std::panic::set_hook(Box::new(|_| {}));
    let mut skips = BTreeMap::new();
    let cases = if suite == "mjsunit" {
        cases_mjsunit(&mjsunit_root(), &mut skips, &mut Vec::new())
    } else {
        cases262(&test262_root(), &mut skips)
    };
    let timeout = timeout();
    let mut out = std::io::stdout();
    let _ = writeln!(out, "{REPLY}{}", Value::String(WORKER_READY.into()));
    let _ = out.flush();
    for line in BufReader::new(std::io::stdin())
        .lines()
        .map_while(Result::ok)
    {
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let case = request["index"]
            .as_u64()
            .and_then(|i| cases.get(i as usize));
        let verdict = match case.map(|c| {
            if !request_matches(c, &request) {
                return Err("worker discovery differs from parent".into());
            }
            run_case(c, timeout)
        }) {
            Some(Ok(())) => Value::Null,
            Some(Err(e)) => Value::String(e),
            None => Value::String("no such case".into()),
        };
        let _ = writeln!(out, "{REPLY}{verdict}");
        let _ = out.flush();
    }
}

fn timeout() -> Duration {
    let millis = match std::env::var("CONF_TIMEOUT_MS") {
        Ok(value) => value
            .parse::<u64>()
            .expect("CONF_TIMEOUT_MS must be a positive duration"),
        Err(_) => 15_000,
    };
    assert!(
        (1..=MAX_CASE_TIMEOUT_MS).contains(&millis),
        "CONF_TIMEOUT_MS must be between 1 and {MAX_CASE_TIMEOUT_MS}"
    );
    Duration::from_millis(millis)
}

fn test262_root() -> PathBuf {
    env_path("TEST262_ROOT", home().join("Dev/upstream/test262"))
}

fn mjsunit_root() -> PathBuf {
    env_path("MJSUNIT_ROOT", home().join("Dev/upstream/v8/test/mjsunit"))
}

/** A private executable snapshot keeps every child on the parent's exact case discovery code. */
struct WorkerBinary(PathBuf);

impl WorkerBinary {
    /** Cargo may replace its test binary during a long run, so workers execute this stable copy. */
    fn snapshot(suite: &str) -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("mars-v9-worker-{suite}-{}-{nonce}", std::process::id()));
        std::fs::OpenOptions::new().write(true).create_new(true).open(&path).expect("reserve worker binary");
        std::fs::copy(std::env::current_exe().unwrap(), &path).expect("snapshot worker binary");
        Self(path)
    }
}

impl Drop for WorkerBinary {
    /** The snapshot is removed only after all worker processes have exited. */
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}

/** Run every case in parallel worker processes, print and write the scoreboard. */
/** mjsunit's own message when `assertOptimized` meets an engine that never optimizes. V9 reports
exactly that through `%GetOptimizationStatus`, so a test ending here ran every assertion before
the optimizer check and proves nothing more; it is counted as a skip, not a pass or a failure. */
const NO_OPTIMIZER_EXIT: &str = "test does not make sense with --no-turbofan";
/** The skip reason for those exits. */
const NO_OPTIMIZER_SKIP: &str = "no-optimizer:assertOptimized";

fn score(
    suite: &'static str,
    cases: Vec<Case>,
    mut skips: BTreeMap<String, usize>,
    mut exclusions: Vec<Value>,
) {
    std::panic::set_hook(Box::new(|_| {}));
    let started = Instant::now();
    let jobs = match std::env::var("CONF_JOBS") {
        Ok(value) => value
            .parse::<usize>()
            .expect("CONF_JOBS must be a positive worker count"),
        Err(_) => thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(MAX_WORKERS),
    };
    assert!(
        (1..=MAX_WORKERS).contains(&jobs),
        "CONF_JOBS must be between 1 and {MAX_WORKERS}"
    );
    let binary = Arc::new(WorkerBinary::snapshot(suite));
    let cases = Arc::new(cases);
    let next = Arc::new(AtomicUsize::new(0));
    let outcomes: Outcomes = Arc::new(Mutex::new((0..cases.len()).map(|_| None).collect()));
    let workers: Vec<_> = (0..jobs)
        .map(|_| {
            let (cases, next, outcomes, binary) = (cases.clone(), next.clone(), outcomes.clone(), binary.clone());
            thread::spawn(move || {
                let mut child: Option<Worker> = None;
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= cases.len() {
                        break;
                    }
                    let result = ask(&mut child, suite, &binary.0, i, &cases[i]);
                    outcomes.lock().unwrap()[i] = Some(result);
                    if i % 2000 == 0 {
                        eprintln!("{suite}: {i}/{}", cases.len());
                    }
                }
                retire(&mut child);
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    let outcomes = outcomes.lock().unwrap();
    let mut groups: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut buckets: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures = Vec::new();
    for (case, outcome) in cases.iter().zip(outcomes.iter()) {
        let entry = groups.entry(case.group.clone()).or_default();
        match outcome.as_ref().unwrap() {
            Ok(()) => entry.0 += 1,
            Err(e) if e.contains(NO_OPTIMIZER_EXIT) => {
                skip_case(&mut skips, &mut exclusions, &case.rel, NO_OPTIMIZER_SKIP.into());
            }
            Err(e) => {
                entry.1 += 1;
                *buckets.entry(bucket(e)).or_default() += 1;
                failures.push(json!({"test": case.rel, "error": e}));
            }
        }
    }
    let (pass, fail) = groups.values().fold((0, 0), |a, g| (a.0 + g.0, a.1 + g.1));
    if suite == "mjsunit" && !include_mjsunit_dirs().is_empty() {
        let skipped = skips.values().sum::<usize>();
        println!(
            "discovered: {}, executed: {}, passed: {pass}, failed: {fail}, skipped: {skipped}",
            pass + fail + skipped,
            pass + fail
        );
    }
    println!(
        "\n{suite}: {pass} pass, {fail} fail, {} skipped ({:.1}s)",
        skips.values().sum::<usize>(),
        started.elapsed().as_secs_f64()
    );
    println!("{:<44} {:>6} {:>6} {:>7}", "group", "pass", "fail", "rate");
    for (name, (p, f)) in &groups {
        println!(
            "{name:<44} {p:>6} {f:>6} {:>6.1}%",
            100.0 * *p as f64 / (*p + *f).max(1) as f64
        );
    }
    let mut top: Vec<_> = buckets.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1));
    println!("\ntop failure reasons:");
    for (reason, n) in top.iter().take(25) {
        println!("{n:>6}  {reason}");
    }
    println!("\nskipped:");
    for (reason, n) in &skips {
        println!("{n:>6}  {reason}");
    }
    let out = out_dir();
    std::fs::create_dir_all(&out).unwrap();
    let groups: BTreeMap<_, Value> = groups
        .iter()
        .map(|(k, (p, f))| (k.clone(), json!({"pass": p, "fail": f})))
        .collect();
    let top: Vec<Value> = top
        .iter()
        .take(100)
        .map(|(r, n)| json!({"reason": r, "count": n}))
        .collect();
    let file = out.join(format!("results-{suite}.json"));
    let body = json!({"suite": suite, "pass": pass, "fail": fail, "discovered":pass + fail + skips.values().sum::<usize>(), "executed":pass + fail, "skipped": skips, "exclusions":exclusions, "groups": groups, "top_failures": top, "failures": failures});
    std::fs::write(&file, serde_json::to_string_pretty(&body).unwrap()).unwrap();
    println!("results in {}", file.display());
    if pass + fail == 0 {
        println!("No tests executed; check MJSUNIT and MJSUNIT_INCLUDE_DIRS.");
    }
    require_pass(
        fail + usize::from(pass + fail == 0),
        std::env::var("CONF_REQUIRE_PASS").as_deref() == Ok("1"),
    );
}

/** A strict conformance gate fails after writing the complete diagnostic scoreboard. */
fn require_pass(fail: usize, required: bool) {
    if required {
        assert_eq!(
            fail, 0,
            "Conformance gate requires every executed case to pass."
        );
    }
}

#[test]
/** Research scoring stays available, but the strict gate never treats case failures as success. */
fn strict_gate_reports_case_failures() {
    require_pass(0, true);
    require_pass(1, false);
    assert!(std::panic::catch_unwind(|| require_pass(1, true)).is_err());
}

/** A worker cannot silently report a reordered or edited case as a pass. */
#[test]
fn worker_requests_bind_path_and_source() {
    let mut case = Case {
        rel: "wasm/a.js".into(),
        group: "wasm".into(),
        runs: vec![("sloppy", "1".into())],
        negative: None,
        is_async: false,
        host: None,
        discovery_error: None,
    };
    let request = json!({"path":case.rel,"identity":case_identity(&case)});
    assert!(request_matches(&case, &request));
    case.runs[0].1 = "2".into();
    assert!(!request_matches(&case, &request));
    case.runs[0].1 = "1".into();
    case.rel = "wasm/b.js".into();
    assert!(!request_matches(&case, &request));
}

/** Upstream status names match standalone files and bounded prefix patterns. */
#[test]
fn status_skip_patterns_match_only_their_paths() {
    assert!(status_matches(
        "wasm/wasm-module-builder",
        "wasm/wasm-module-builder.js"
    ));
    assert!(status_matches("wasm/fixture*", "wasm/fixture-a.js"));
    assert!(!status_matches("wasm/fixture*", "wasm/other.js"));
    let text = "[ALWAYS, {\n 'modules-skip*': [SKIP],\n 'run': [PASS],\n}],  # ALWAYS";
    let entries = always_status_entries(text, "[SKIP]");
    assert!(
        entries
            .iter()
            .any(|pattern| status_matches(pattern, "modules-skip-reset1.js"))
    );
    assert!(
        !entries
            .iter()
            .any(|pattern| status_matches(pattern, "run.js"))
    );
}


/** The SIMD condition is read from its exact status block, without admitting other conditions. */
#[test]
fn simd_status_skips_only_matching_block() {
    let status = "['no_simd_hardware == True', {\n  'wasm/keep': [SKIP],\n}],  # no_simd_hardware == True\n['no_simd_hardware == False', {\n  'regress/wasm/regress-1254675': [SKIP],\n  'wasm/prefix-*': [SKIP],\n}],  # no_simd_hardware == False\n";
    let skips = simd_status_entries(status);
    assert!(
        skips
            .iter()
            .any(|pattern| status_matches(pattern, "regress/wasm/regress-1254675.js"))
    );
    assert!(
        skips
            .iter()
            .any(|pattern| status_matches(pattern, "wasm/prefix-one.js"))
    );
    assert!(
        !skips
            .iter()
            .any(|pattern| status_matches(pattern, "wasm/keep.js"))
    );
    assert!(simd_status_entries("['other', {'x': [SKIP]}]").is_empty());
}

/** Platform exclusions apply only when V8's exact non-Linux condition is true. */
#[test]
fn non_linux_status_skips_only_matching_platform_and_block() {
    let status = "['system == linux', {\n 'wasm/linux-only': [SKIP],\n}],\n['system != linux', {\n 'wasm/memory-control': [SKIP],\n 'regress/wasm/regress-493307331': [SKIP],\n 'wasm/keep': [PASS],\n}],\n";
    assert!(non_linux_status_entries(status, true).is_empty());
    let skips = non_linux_status_entries(status, false);
    assert!(skips.iter().any(|name| status_matches(name, "wasm/memory-control.js")));
    assert!(skips.iter().any(|name| status_matches(name, "regress/wasm/regress-493307331.js")));
    assert!(!skips.iter().any(|name| status_matches(name, "wasm/linux-only.js")));
    assert!(!skips.iter().any(|name| status_matches(name, "wasm/keep.js")));
}

/** tc39 test262 against V9. */
#[test]
#[ignore]
fn test262() {
    let root = test262_root();
    let mut skips = BTreeMap::new();
    let cases = cases262(&root, &mut skips);
    score("test262", cases, skips, Vec::new());
}

/** V8's mjsunit against V9. */
#[test]
#[ignore]
fn mjsunit() {
    let root = mjsunit_root();
    let mut skips = BTreeMap::new();
    let mut exclusions = Vec::new();
    let cases = cases_mjsunit(&root, &mut skips, &mut exclusions);
    score("mjsunit", cases, skips, exclusions);
}

/** The frontmatter reader handles flow and block lists and the negative block, an unclaimed
feature skips a test, and Temporal (claimed since round 7) runs. */
#[test]
fn frontmatter_parses() {
    let m = meta(
        "/*---\nincludes: [a.js, b.js]\nflags:\n  - onlyStrict\nnegative:\n  phase: parse\n  type: SyntaxError\nfeatures: [ShadowRealm]\n---*/",
    );
    assert_eq!(m.includes, ["a.js", "b.js"]);
    assert_eq!(m.flags, ["onlyStrict"]);
    assert_eq!(m.negative, Some(("parse".into(), "SyntaxError".into())));
    assert_eq!(
        skip262("built-ins/x.js", &m).as_deref(),
        Some("feature:ShadowRealm")
    );
    let temporal = meta("/*---\nfeatures: [Temporal, Intl.NumberFormat-v3]\n---*/");
    assert_eq!(skip262("built-ins/Temporal/x.js", &temporal), None);
}
