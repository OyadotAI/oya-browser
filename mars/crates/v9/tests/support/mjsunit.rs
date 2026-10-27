/** V8's test-host setup: separate global Scripts, declared Files, and inert d8 fixtures.
Upstream source and assertions are always passed unchanged to the engine. */
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;
mod bundle;
mod data_modules;
#[cfg(feature = "conformance")]
#[path = "mjsunit/profiles.rs"]
mod profiles;
mod timezone;

/** A runaway interval cannot turn an upstream case into an unbounded host event loop. */
const TIMER_CALLBACK_LIMIT: usize = 1024;
/** One task per delivery preserves timer and microtask ordering and makes the bound exact. */
const TIMER_CALLBACKS_PER_STEP: usize = 1;

/** Sources are separate to preserve directives, lexical scopes and Script instantiation. */
#[derive(Clone)]
pub(super) struct Scripts {
    /** Setup failures are test failures, never skipped cases. */
    sources: Result<Vec<String>, String>,
    /** The canonical checkout root confines fixture access to the upstream checkout. */
    root: PathBuf,
    /** The test and its prerequisites seed recursive fixture discovery. */
    fixtures: BTreeMap<String, String>,
    /** Source and binary fixtures preserve their original byte content independently of text decoding. */
    files: BTreeMap<String, Vec<u8>>,
    /** The original test's file URL resolves relative dynamic imports without changing source. */
    test_url: String,
    /** Metadata belongs to the original container even when native bundle parts execute separately. */
    metadata_source: String,
    /** Classic sources carry their actual host filenames independently of source text. */
    source_names: Vec<String>,
    /** The supplied harness belongs to its actual checkout path. */
    harness_url: String,
    /** All bundle modules are registered before any ordered Script or module entrypoint executes. */
    bundle: Option<bundle::Bundle>,
    /** Logical module entrypoint identities remain distinct from the physical container filename. */
    bundle_entries: BTreeMap<usize, String>,
}

impl Scripts {
    /** Mirrors test/mjsunit/testcfg.py's harness, Files, then test order. */
    pub(super) fn new(root: &Path, harness: &str, source: &str) -> Self {
        Self::for_test(root, harness, source, &root.join("test.js"))
    }

    /** Preserve the upstream file's actual location for module resolution in any realm. */
    pub(super) fn for_test(root: &Path, harness: &str, source: &str, file: &Path) -> Self {
        let checkout = root.parent().and_then(Path::parent).unwrap_or(root);
        let mut scripts = Self {
            sources: Ok(Vec::new()),
            root: checkout.to_path_buf(),
            fixtures: BTreeMap::new(),
            files: BTreeMap::new(),
            test_url: url::Url::from_file_path(file)
                .map(|url| url.to_string())
                .unwrap_or_default(),
            metadata_source: source.into(),
            source_names: Vec::new(),
            harness_url: url::Url::from_file_path(root.join("mjsunit.js"))
                .map(|url| url.to_string())
                .unwrap_or_default(),
            bundle: None,
            bundle_entries: BTreeMap::new(),
        };
        scripts.sources = scripts.sources(harness, source);
        scripts.discover(source);
        scripts
    }

    /** Native d8 output needs no JavaScript shim; the empty bootstrap slot preserves source-name alignment. */
    fn sources(&mut self, harness: &str, source: &str) -> Result<Vec<String>, String> {
        #[cfg(not(feature = "conformance"))]
        let mut sources =
            vec!["var print = function (value) { console.log(String(value)); };".into()];
        #[cfg(feature = "conformance")]
        let mut sources = vec![String::new()];
        self.source_names.push(String::new());
        if !source.lines().any(|line| line.trim() == "// NO HARNESS") {
            sources.push(harness.into());
            self.source_names.push(self.harness_url.clone());
        }
        for name in declared_files(source) {
            sources.push(self.read(&name)?);
            self.source_names.push(
                url::Url::from_file_path(self.root.join(name))
                    .map(|url| url.to_string())
                    .unwrap_or_default(),
            );
        }
        if boolean_flag(source, "bundle")
            && let Some(bundle) = bundle::parse(source, &self.root)?
        {
            for item in &bundle.items {
                match item {
                    bundle::Item::Script(source) => {
                        sources.push(source.clone());
                        self.source_names.push(self.test_url.clone());
                    }
                    bundle::Item::Module(url) => {
                        self.bundle_entries.insert(sources.len(), url.clone());
                        sources.push(bundle.modules[url].clone());
                        self.source_names.push(url.clone());
                    }
                }
            }
            self.bundle = Some(bundle);
        } else {
            sources.push(source.into());
            self.source_names.push(self.test_url.clone());
        }
        Ok(sources)
    }

    /** Canonical paths prevent a fixture or symlink from reading outside the checkout. */
    fn read(&mut self, name: &str) -> Result<String, String> {
        let root = self
            .root
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let path = root
            .join(name)
            .canonicalize()
            .map_err(|error| format!("fixture {name}: {error}"))?;
        if !path.starts_with(&root) {
            return Err(format!("fixture {name} escapes upstream checkout"));
        }
        let mut bytes = Vec::new();
        let limit = mars_v9::constants::MODULE_BYTES;
        std::fs::File::open(path)
            .map_err(|error| format!("fixture {name}: {error}"))?
            .take((limit + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("fixture {name}: {error}"))?;
        let resident: usize = self
            .files
            .iter()
            .filter(|(path, _)| path.as_str() != name)
            .map(|(_, bytes)| bytes.len())
            .sum();
        if bytes.len() > limit.saturating_sub(resident) {
            return Err("fixture byte registry exceeds the module byte bound".into());
        }
        self.files.insert(name.into(), bytes.clone());
        let text = String::from_utf8(bytes).map_err(|error| format!("fixture {name}: {error}"))?;
        self.fixtures.insert(name.into(), text.clone());
        Ok(text)
    }

    /** Recursively provide literal fixture references; unavailable capabilities still fail. */
    fn discover(&mut self, source: &str) {
        if !cfg!(feature = "conformance") {
            return;
        }
        let mut pending: Vec<String> = self.fixtures.values().cloned().collect();
        pending.push(source.to_string());
        let mut seen = BTreeSet::new();
        while let Some(source) = pending.pop() {
            for name in fixture_names(&source) {
                if seen.insert(name.clone())
                    && let Ok(text) = self.read(&name)
                {
                    pending.push(text);
                }
            }
        }
    }

    /** Run every source exactly as supplied, and surface the first failed Script. */
    pub(super) fn execute(&self) -> Result<(), String> {
        let sources = self.sources.as_ref().map_err(Clone::clone)?;
        let host = crate::common::host();
        #[cfg(feature = "conformance")]
        let workers = mars_v9::ConformanceWorkerSession::new();
        let mut environment = crate::common::environment();
        environment.scripts = self.source_names.clone();
        environment.url = self.test_url.clone();
        environment.regexp_buffer_boundaries =
            boolean_flag(&self.metadata_source, "js-regexp-buffer-boundaries");
        environment.disallow_code_generation_from_strings = boolean_flag(
            &self.metadata_source,
            "disallow-code-generation-from-strings",
        );
        if let Some(zone) = timezone::load(&self.metadata_source)? {
            environment.timezone_initial_offset_seconds = Some(zone.initial);
            environment.timezone_transitions = zone.transitions;
            environment.timezone_posix_rule = zone.posix;
            environment.timezone_display_names = zone.display_names;
            environment.timezone_daylight = zone.daylight;
            environment.timezone_initial_daylight = zone.initial_daylight;
        }
        #[cfg(feature = "conformance")]
        let environment = mars_v9::PageEnvironment {
            conformance_sources: self.fixtures.clone(),
            conformance_files: self.files.clone(),
            conformance_d8: true,
            conformance_allow_unsafe_function_constructor: boolean_flag(
                &self.metadata_source,
                "allow-unsafe-function-constructor",
            ),
            conformance_cpu_trace_name: string_flag(
                &self.metadata_source,
                "expose-cputracemark-as",
            )?,
            conformance_random_seed: random_seed_flag(&self.metadata_source)?,
            conformance_cpu_profiler_interval_us: profile_interval(&self.metadata_source)?,
            conformance_fuzzing: boolean_flag(&self.metadata_source, "fuzzing"),
            conformance_expose_fast_api: boolean_flag(&self.metadata_source, "expose-fast-api")
                && boolean_flag_default(&self.metadata_source, "turbo-fast-api-calls", true)
                && !boolean_flag(&self.metadata_source, "correctness-fuzzer-suppressions"),
            conformance_async_hooks: boolean_flag(&self.metadata_source, "expose-async-hooks"),
            conformance_inspector: boolean_flag(&self.metadata_source, "enable-inspector"),
            conformance_harmony_struct: boolean_flag(&self.metadata_source, "harmony-struct"),
            conformance_experimental_stack_trace_frames: boolean_flag(
                &self.metadata_source,
                "experimental-stack-trace-frames",
            ),
            conformance_async_stack_traces: boolean_flag_default(
                &self.metadata_source,
                "async-stack-traces",
                true,
            ),
            conformance_builtin_subclassing: boolean_flag_default(
                &self.metadata_source,
                "builtin-subclassing",
                true,
            ),
            conformance_iterator_includes: boolean_flag_default(
                &self.metadata_source,
                "js-iterator-includes",
                boolean_flag(&self.metadata_source, "js-staging")
                    || boolean_flag(&self.metadata_source, "harmony"),
            ),
            conformance_iterator_join: boolean_flag_default(
                &self.metadata_source,
                "js-iterator-join",
                boolean_flag(&self.metadata_source, "js-staging")
                    || boolean_flag(&self.metadata_source, "harmony"),
            ),
            conformance_ignore_unhandled_promises: boolean_flag(
                &self.metadata_source,
                "ignore-unhandled-promises",
            ),
            conformance_workers: Some(workers.host()),
            conformance_wasm_max_module_bytes: wasm_module_limit(&self.metadata_source)?,
            conformance_wasm_max_table_entries: numeric_flag_limit(
                &self.metadata_source,
                "--wasm-max-table-size",
            )?,
            conformance_expose_statistics: boolean_flag(&self.metadata_source, "expose-statistics"),
            conformance_expose_trigger_failure: boolean_flag(
                &self.metadata_source,
                "expose-trigger-failure",
            ),
            conformance_enable_tracing: boolean_flag(&self.metadata_source, "enable-tracing"),
            conformance_log: boolean_flag(&self.metadata_source, "log"),
            ..environment
        };
        let modules = self.modules();
        let input = mars_v9::PageInput {
            modules: &modules,
            ..crate::common::input(sources, &host, &environment)
        };
        let expected_throw = boolean_flag(&self.metadata_source, "throws");
        let result = match mars_v9::load_page(copy_input(&input)) {
            Ok(execution) => settle(self, input, execution, expected_throw),
            Err(error) => terminal_error(&error, expected_throw),
        };
        #[cfg(feature = "conformance")]
        let result = result.and(
            workers
                .finish()
                .map_err(|error| format!("Worker {}: {}", error.code, error.message)),
        );
        #[cfg(feature = "conformance")]
        let result = result.and(profiles::write(&self.test_url, workers.take_cpu_profiles()));
        result
    }

    /** Literal shell data modules and confined checkout files carry real source; other requests fail fetching. */
    fn module_source(&self, request: &mars_v9::ModuleRequest) -> mars_v9::ModuleSource {
        let source = data_modules::source(&request.url).or_else(|| {
            url::Url::parse(&request.url)
                .ok()
                .and_then(|url| url.to_file_path().ok())
                .and_then(|path| path.canonicalize().ok())
                .filter(|path| {
                    self.root
                        .canonicalize()
                        .is_ok_and(|root| path.starts_with(root))
                })
                .and_then(|path| std::fs::read_to_string(path).ok())
        });
        mars_v9::ModuleSource {
            url: request.url.clone(),
            source,
            classic: false,
        }
    }

    /** Precompiled classic fixture code has stable offsets and source-derived restored globals. */
    fn modules(&self) -> mars_v9::ModuleInput {
        let mut modules = mars_v9::ModuleInput::default();
        if cfg!(feature = "conformance") {
            modules.sources = self
                .fixtures
                .iter()
                .map(|(name, source)| mars_v9::ModuleSource {
                    url: name.clone(),
                    source: Some(source.clone()),
                    classic: true,
                })
                .collect();
        }
        if let Some(bundle) = &self.bundle {
            modules
                .sources
                .extend(
                    bundle
                        .modules
                        .iter()
                        .map(|(url, source)| mars_v9::ModuleSource {
                            url: url.clone(),
                            source: Some(source.clone()),
                            classic: false,
                        }),
                );
            modules
                .entries
                .extend(
                    self.bundle_entries
                        .iter()
                        .map(|(index, url)| mars_v9::ModuleEntry {
                            index: *index,
                            kind: mars_v9::EntryKind::Module,
                            url: Some(url.clone()),
                        }),
                );
        }
        modules
    }
}

/** Timers are real V9 callbacks on its virtual clock; no wall-clock sleeps or callback mocks. */
fn settle(
    scripts: &Scripts,
    input: mars_v9::PageInput<'_>,
    mut execution: mars_v9::Execution,
    expected_throw: bool,
) -> Result<(), String> {
    let deadline = Instant::now() + crate::timeout();
    let mut modules = input.modules.clone();
    for _ in 0..TIMER_CALLBACK_LIMIT {
        if check(&execution, expected_throw)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("host event loop time limit exceeded".into());
        }
        if !execution.module_requests.is_empty() {
            modules.sources.extend(
                execution
                    .module_requests
                    .iter()
                    .map(|request| scripts.module_source(request)),
            );
            let delivery = mars_v9::PageInput {
                modules: &modules,
                ..copy_input(&input)
            };
            execution = match mars_v9::load_modules(delivery, execution.state) {
                Ok(execution) => execution,
                Err(error) => return terminal_error(&error, expected_throw),
            };
            continue;
        }
        let Some(due) = execution.effects.next_timer_ms else {
            return completed(expected_throw);
        };
        execution = match mars_v9::run_timers(
            mars_v9::PageInput {
                modules: &modules,
                ..copy_input(&input)
            },
            execution.state,
            due,
            TIMER_CALLBACKS_PER_STEP,
        ) {
            Ok(execution) => execution,
            Err(error) => return terminal_error(&error, expected_throw),
        };
    }
    if check(&execution, expected_throw)? {
        return Ok(());
    }
    if !execution.module_requests.is_empty() {
        return Err("host event loop module delivery limit exceeded".into());
    }
    match execution.effects.next_timer_ms {
        Some(_) => Err("host event loop callback limit exceeded".into()),
        None => completed(expected_throw),
    }
}

/** A real host exit ends script and task delivery; every nonzero status remains a failed test. */
fn test_exit(error: &mars_v9::protocol::EngineError) -> Option<Result<(), String>> {
    if error.code != "conformance_exit" {
        return None;
    }
    Some(if error.message == "0" {
        Ok(())
    } else {
        Err(format!(
            "Test host exited with status {}\n{}",
            error.message, error.hint
        ))
    })
}

/** Timer deliveries reuse identical source and host facts while owning their inert projection. */
fn copy_input<'a>(input: &mars_v9::PageInput<'a>) -> mars_v9::PageInput<'a> {
    mars_v9::PageInput {
        sources: input.sources,
        source_keys: &[],
        document: input.document.clone(),
        storage: input.storage.clone(),
        host: input.host,
        environment: input.environment,
        modules: input.modules,
    }
}

/** The explicit engine exception channel cannot confuse a console.error call with a throw. */
fn check(execution: &mars_v9::Execution, expected_throw: bool) -> Result<bool, String> {
    #[cfg(feature = "conformance")]
    if let Some((code, message)) = &execution.effects.conformance_native_error {
        return Err(format!("{code}: {message}"));
    }
    if let Some(failure) = execution.failed.first() {
        terminal_error(&failure.error, expected_throw)?;
        return Ok(true);
    }
    #[cfg(feature = "conformance")]
    if let Some(error) = &execution.effects.conformance_error {
        if expected_throw && !error.contains("MjsUnitAssertionError") {
            return Ok(true);
        }
        return Err(error.clone());
    }
    Ok(false)
}

/** A genuine language exception satisfies --throws; assertions, unsupported features and native failures never do. */
fn terminal_error(
    error: &mars_v9::protocol::EngineError,
    expected_throw: bool,
) -> Result<(), String> {
    if let Some(result) = test_exit(error) {
        return result.and_then(|()| completed(expected_throw));
    }
    if expected_throw
        && matches!(
            error.code.as_str(),
            "script_execution_error" | "script_syntax_error"
        )
        && !error.message.contains("MjsUnitAssertionError")
    {
        return Ok(());
    }
    Err(format!("{}: {}", error.code, error.message))
}

/** --throws also requires an actual uncaught exception; ordinary successful completion must fail. */
fn completed(expected_throw: bool) -> Result<(), String> {
    if expected_throw {
        Err("Expected an uncaught JavaScript exception (--throws), but execution completed.".into())
    } else {
        Ok(())
    }
}

/** V8 accepts several whitespace-separated Files directives, preserving their order. */
fn declared_files(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("//")?
                .trim_start()
                .strip_prefix("Files:")
        })
        .flat_map(str::split_whitespace)
        .map(str::to_string)
        .collect()
}

/** Sampling metadata is an actual positive native microsecond interval, not an instruction count. */
#[cfg(feature = "conformance")]
fn profile_interval(source: &str) -> Result<u64, String> {
    let interval = numeric_flag_limit(source, "--cpu-profiler-sampling-interval")?.unwrap_or(1_000);
    if interval == 0 || i32::try_from(interval).is_err() {
        return Err("Invalid --cpu-profiler-sampling-interval value".into());
    }
    Ok(interval as u64)
}

/** A real V8 limit flag may only lower the native engine's existing module-byte bound. */
fn wasm_module_limit(source: &str) -> Result<Option<usize>, String> {
    numeric_flag_limit(source, "--wasm-max-module-size")
}
/** Lower-only native bounds use actual numeric flag spelling and command-line override order. */
fn numeric_flag_limit(source: &str, name: &str) -> Result<Option<usize>, String> {
    let prefix = format!("{name}=");
    let mut flags = source
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("//")?
                .trim_start()
                .strip_prefix("Flags:")
        })
        .flat_map(str::split_whitespace);
    let mut maximum = None;
    while let Some(flag) = flags.next() {
        let value = if flag == name {
            Some(
                flags
                    .next()
                    .ok_or_else(|| format!("Missing {name} value"))?,
            )
        } else {
            flag.strip_prefix(&prefix)
        };
        if let Some(value) = value {
            maximum = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| format!("Invalid {name} value"))?,
            );
        }
    }
    Ok(maximum)
}

/** Positive and negative host switches apply in metadata order, with V8's underscore spelling alias. */
fn boolean_flag(source: &str, name: &str) -> bool {
    boolean_flag_default(source, name, false)
}

/** V8's signed decimal int seed flag keeps zero distinct from the absence of explicit metadata. */
#[cfg(feature = "conformance")]
fn random_seed_flag(source: &str) -> Result<Option<i32>, String> {
    string_flag(source, "random-seed")?
        .map(|seed| {
            seed.parse::<i32>()
                .map_err(|_| "Invalid --random-seed value".to_string())
        })
        .transpose()
}

/** String flags preserve their actual values and last override while normalizing only the flag key's underscore alias. */
#[cfg(feature = "conformance")]
fn string_flag(source: &str, name: &str) -> Result<Option<String>, String> {
    let expected = format!("--{name}");
    let mut flags = source
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("//")?
                .trim_start()
                .strip_prefix("Flags:")
        })
        .flat_map(str::split_whitespace);
    let mut value = None;
    while let Some(flag) = flags.next() {
        let (key, inline) = flag
            .split_once('=')
            .map_or((flag, None), |(key, value)| (key, Some(value)));
        if key.replace('_', "-") == expected {
            let text = inline
                .or_else(|| flags.next())
                .ok_or_else(|| format!("Missing {expected} value"))?;
            value = Some(text.to_string());
        }
    }
    Ok(value)
}
/** Flags with a primary default of true retain that default when metadata omits the switch. */
fn boolean_flag_default(source: &str, name: &str, default: bool) -> bool {
    let mut enabled = default;
    for flag in source
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("//")?
                .trim_start()
                .strip_prefix("Flags:")
        })
        .flat_map(str::split_whitespace)
    {
        let flag = flag.replace('_', "-");
        if flag == format!("--{name}") || flag == format!("--{name}=true") {
            enabled = true;
        } else if flag == format!("--no-{name}")
            || flag == format!("--no{name}")
            || flag == format!("--{name}=false")
        {
            enabled = false;
        }
    }
    enabled
}

/** Literal checkout paths cover d8.file.execute/read prerequisites without rewriting calls.
Computed paths absent from this registry fail explicitly instead of returning dummy source. */
fn fixture_names(source: &str) -> Vec<String> {
    source
        .match_indices("test/")
        .filter_map(|(at, _)| {
            let quote = *source.as_bytes().get(at.checked_sub(1)?)?;
            if quote != b'\'' && quote != b'"' {
                return None;
            }
            let rest = &source[at..];
            let end = rest.find(quote as char)?;
            (!rest[..end].contains(['\\', '\n', '\r'])).then(|| rest[..end].to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /** Profiler metadata forwards real microseconds and rejects invalid native flag values. */
    #[cfg(feature = "conformance")]
    #[test]
    fn profiler_sampling_flags_preserve_real_interval_and_override_order() {
        assert_eq!(profile_interval(""), Ok(1_000));
        assert_eq!(
            profile_interval(
                "// Flags: --cpu-profiler-sampling-interval=10\n// Flags: --cpu-profiler-sampling-interval 1000000"
            ),
            Ok(1_000_000)
        );
        for source in [
            "// Flags: --cpu-profiler-sampling-interval=0",
            "// Flags: --cpu-profiler-sampling-interval=-1",
            "// Flags: --cpu-profiler-sampling-interval=2147483648",
            "// Flags: --cpu-profiler-sampling-interval=nope",
        ] {
            assert!(profile_interval(source).is_err());
        }
    }

    /** A no-listener shell session delivers actual sampled JSON into the confined host-owned v8.prof artifact. */
    #[cfg(feature = "conformance")]
    #[test]
    fn profiler_without_listener_writes_real_sample_artifact_through_the_runner() {
        let checkout = Checkout::new();
        let scripts = checkout.scripts("function artifactSample(){d8.profiler.triggerSample()}console.profile();artifactSample();console.profileEnd();", "");
        assert_eq!(scripts.execute(), Ok(()));
        let digest = ring::digest::digest(&ring::digest::SHA256, scripts.test_url.as_bytes());
        let name: String = digest
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let file = crate::out_dir()
            .join("cpu-profiles")
            .join(name)
            .join("root/v8.prof");
        let json = std::fs::read_to_string(file).expect("real host artifact exists");
        let profile: serde_json::Value =
            serde_json::from_str(&json).expect("actual CpuProfile JSON");
        assert!(profile["samples"].as_array().unwrap().len() >= 2);
        assert!(
            profile["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["callFrame"]["functionName"] == "artifactSample")
        );
        assert!(profile["endTime"].as_u64().unwrap() >= profile["startTime"].as_u64().unwrap());
    }

    /** Literal data source reaches the real compiler and its exported value reaches the importer. */
    #[cfg(feature = "conformance")]
    #[test]
    fn data_module_source_is_compiled_and_evaluated() {
        let checkout = Checkout::new();
        assert_eq!(checkout.scripts("import('data:text/javascript,export const answer=6*7').then(ns=>{if(ns.answer!==42)throw Error('wrong export')});", "").execute(), Ok(()));
    }

    /** Newline-sensitive comments, private fields and percent text remain literal module bytes. */
    #[cfg(feature = "conformance")]
    #[test]
    fn data_module_source_is_not_url_normalized_or_decoded() {
        let checkout = Checkout::new();
        assert_eq!(checkout.scripts("import(\"data:text/javascript,// comment\\nexport class Box{#x=7;get(){return this.#x}};export const text='%23';\").then(ns=>{if(new ns.Box().get()!==7||ns.text!=='%23')throw Error('changed literal source')});", "").execute(), Ok(()));
    }

    /** Nested imports are genuine later host requests and preserve their own literal module identities. */
    #[cfg(feature = "conformance")]
    #[test]
    fn nested_data_modules_complete_through_real_module_delivery() {
        let checkout = Checkout::new();
        assert_eq!(checkout.scripts("const inner='data:text/javascript,export default 9';const outer='data:text/javascript,export const ready=import('+JSON.stringify(inner)+')';import(outer).then(ns=>ns.ready).then(ns=>{if(ns.default!==9)throw Error('wrong nested export')});", "").execute(), Ok(()));
    }

    /** Real module exceptions remain failures unless JavaScript explicitly handles that import rejection. */
    #[cfg(feature = "conformance")]
    #[test]
    fn data_module_exceptions_are_not_converted_into_success() {
        let checkout = Checkout::new();
        let failure = checkout
            .scripts(
                "import('data:text/javascript,throw Error(\"literal module failure\")');",
                "",
            )
            .execute()
            .expect_err("uncaught module failure");
        assert!(failure.contains("literal module failure"), "{failure}");
        assert_eq!(checkout.scripts("import('data:text/javascript,throw Error(\"literal module failure\")').catch(e=>{if(e.message!=='literal module failure')throw e});", "").execute(), Ok(()));
    }

    /** Signed seed values and policy toggles follow original metadata order without filename rules. */
    #[cfg(feature = "conformance")]
    #[test]
    fn seeded_rng_and_codegen_flags_preserve_values_and_last_overrides() {
        assert_eq!(
            random_seed_flag("// Flags: --random_seed -7 --random-seed=12345"),
            Ok(Some(12345))
        );
        assert_eq!(random_seed_flag("// Flags: --random-seed=0"), Ok(Some(0)));
        assert_eq!(random_seed_flag("// Flags: --other"), Ok(None));
        assert!(random_seed_flag("// Flags: --random-seed=2147483648").is_err());
        assert!(!boolean_flag(
            "// Flags: --disallow-code-generation-from-strings --no-disallow_code_generation_from_strings",
            "disallow-code-generation-from-strings"
        ));
    }

    /** quit cannot be caught, cannot continue the source, and prevents pending callbacks. */
    #[cfg(feature = "conformance")]
    #[test]
    fn quit_ends_source_and_task_delivery() {
        let checkout = Checkout::new();
        let scripts = checkout.scripts("setTimeout(()=>{throw Error('late')},0);try{quit(0)}catch(e){throw Error('caught')}throw Error('continued');", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    /** Host interruption stops sources and pending tasks without evaluating ignored arguments. */
    #[cfg(feature = "conformance")]
    #[test]
    fn host_termination_is_uncatchable() {
        let checkout = Checkout::new();
        for method in ["terminate", "terminateNow"] {
            let source = format!(
                "setTimeout(()=>{{throw Error('late')}},0);try{{d8.{method}({{valueOf(){{throw Error('argument')}}}})}}catch(e){{throw Error('caught')}}throw Error('continued');"
            );
            assert_eq!(checkout.scripts(&source, "").execute(), Ok(()));
        }
    }

    /** A quit from a task ends task delivery while preserving a nonzero failure status. */
    #[cfg(feature = "conformance")]
    #[test]
    fn quit_from_tasks_preserves_exit_status() {
        let checkout = Checkout::new();
        assert_eq!(checkout.scripts("setTimeout(()=>{quit(0);throw Error('continued')},0);setTimeout(()=>{throw Error('late')},0);", "").execute(), Ok(()));
        assert!(
            checkout
                .scripts("quit(7);", "")
                .execute()
                .unwrap_err()
                .contains("status 7")
        );
    }

    /** Every test owns its scratch tree; no upstream checkout or shared fixture is written. */
    struct Checkout(PathBuf);
    impl Checkout {
        /** A unique checkout-shaped tree, including the standard mjsunit directory. */
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "mars-mjsunit-host-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(path.join("checkout/test/mjsunit")).unwrap();
            Self(path)
        }
        /** The only root the host is allowed to read. */
        fn root(&self) -> PathBuf {
            self.0.join("checkout/test/mjsunit")
        }
        /** Build the exact host setup used by the real upstream runner. */
        fn scripts(&self, source: &str, harness: &str) -> Scripts {
            Scripts::new(&self.root(), harness, source)
        }
    }
    impl Drop for Checkout {
        /** Remove only the unique scratch data this test created. */
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    /** Actual module requests recursively deliver unchanged files relative to each importer. */
    fn dynamic_modules_resolve_nested_relative_imports() {
        let checkout = Checkout::new();
        std::fs::create_dir_all(checkout.root().join("nested")).unwrap();
        std::fs::write(
            checkout.root().join("nested/first.mjs"),
            "export {value} from './second.mjs';",
        )
        .unwrap();
        std::fs::write(
            checkout.root().join("nested/second.mjs"),
            "export const value = 42;",
        )
        .unwrap();
        let scripts = checkout.scripts("let complete = false; import('./nested/first.mjs').then(module=>{if(module.value!==42)throw Error('wrong export');complete=true});setTimeout(()=>{if(!complete)throw Error('not evaluated')},0);", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[test]
    /** Missing files are delivered as failed fetches and reject rather than resolving empty modules. */
    fn missing_dynamic_modules_reject() {
        let checkout = Checkout::new();
        let scripts = checkout.scripts("let rejected = false; import('./absent.mjs').catch(()=>{rejected=true});setTimeout(()=>{if(!rejected)throw Error('missing module resolved')},0);", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Errors thrown by real module evaluation retain their asynchronous failure outcome. */
    fn module_evaluation_errors_fail_the_case() {
        let checkout = Checkout::new();
        std::fs::write(
            checkout.root().join("error.mjs"),
            "throw Error('module failure');",
        )
        .unwrap();
        assert!(
            checkout
                .scripts("import('./error.mjs');", "")
                .execute()
                .unwrap_err()
                .contains("module failure")
        );
    }

    #[test]
    /** Canonical module reads cannot traverse beyond the checkout, even for existing source. */
    fn dynamic_modules_cannot_escape_the_checkout() {
        let checkout = Checkout::new();
        std::fs::write(checkout.0.join("outside.mjs"), "export const secret = 42;").unwrap();
        let scripts = checkout.scripts("let rejected = false; import('../../../outside.mjs').catch(()=>{rejected=true});setTimeout(()=>{if(!rejected)throw Error('outside source was evaluated')},0);", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[cfg(unix)]
    #[test]
    /** A symlink grants no module read authority outside the configured checkout. */
    fn dynamic_module_symlinks_cannot_escape_the_checkout() {
        let checkout = Checkout::new();
        std::fs::write(checkout.0.join("outside.mjs"), "export const secret = 42;").unwrap();
        std::os::unix::fs::symlink(
            checkout.0.join("outside.mjs"),
            checkout.root().join("escape.mjs"),
        )
        .unwrap();
        let scripts = checkout.scripts("let rejected = false; import('./escape.mjs').catch(()=>{rejected=true});setTimeout(()=>{if(!rejected)throw Error('outside source was evaluated')},0);", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[test]
    /** All Files lines are parsed in their declaration order. */
    fn files_directives_preserve_order() {
        assert_eq!(
            declared_files("// Files: a.js b.js\n//  Files: c.js\n"),
            ["a.js", "b.js", "c.js"]
        );
    }

    #[test]
    /** Only actual Flags metadata supplies this lower bound, with normal command-line override order. */
    fn module_byte_flags_preserve_actual_metadata_semantics() {
        assert_eq!(
            wasm_module_limit(
                "// Flags: --wasm-max-module-size=100\n// Flags: --other --wasm-max-module-size 80"
            ),
            Ok(Some(80))
        );
        assert_eq!(
            wasm_module_limit("// Flags: --wasm-max-module-size=0"),
            Ok(Some(0))
        );
        assert_eq!(
            wasm_module_limit("const text='--wasm-max-module-size=100';"),
            Ok(None)
        );
        for source in [
            "// Flags: --wasm-max-module-size",
            "// Flags: --wasm-max-module-size=-1",
            "// Flags: --wasm-max-module-size=invalid",
        ] {
            assert!(wasm_module_limit(source).is_err());
        }
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Only real Flags metadata selects an extension name, preserving underscores inside that name and override order. */
    fn cpu_marker_string_flags_preserve_the_actual_name() {
        assert_eq!(
            string_flag(
                "// Flags: --expose_cputracemark_as=first_name\n// Flags: --expose-cputracemark-as second_name",
                "expose-cputracemark-as"
            ),
            Ok(Some("second_name".into()))
        );
        assert_eq!(
            string_flag(
                "// Flags: --expose-cputracemark-as=",
                "expose-cputracemark-as"
            ),
            Ok(Some(String::new()))
        );
        assert_eq!(
            string_flag(
                "const text='--expose-cputracemark-as=ignored';",
                "expose-cputracemark-as"
            ),
            Ok(None)
        );
        assert!(
            string_flag(
                "// Flags: --expose-cputracemark-as",
                "expose-cputracemark-as"
            )
            .is_err()
        );
    }

    #[test]
    /** Hook switches use actual metadata, support negative aliases, and follow override order. */
    fn boolean_flags_preserve_actual_metadata_semantics() {
        assert!(!boolean_flag_default(
            "// Flags: --noasync-stack-traces",
            "async-stack-traces",
            true
        ));
        assert!(boolean_flag_default(
            "// Flags: --noasync-stack-traces --async_stack_traces",
            "async-stack-traces",
            true
        ));
        assert!(boolean_flag_default("", "builtin-subclassing", true));
        assert!(!boolean_flag_default(
            "// Flags: --no-builtin-subclassing",
            "builtin-subclassing",
            true
        ));
        assert!(boolean_flag_default(
            "// Flags: --no-builtin-subclassing --builtin-subclassing",
            "builtin-subclassing",
            true
        ));
        assert!(boolean_flag(
            "// Flags: --expose_async_hooks",
            "expose-async-hooks"
        ));
        assert!(!boolean_flag(
            "// Flags: --expose-async-hooks --no-expose-async-hooks",
            "expose-async-hooks"
        ));
        assert!(boolean_flag(
            "// Flags: --expose-async-hooks=false\n// Flags: --expose-async-hooks=true",
            "expose-async-hooks"
        ));
        assert!(!boolean_flag(
            "const text='--expose-async-hooks'",
            "expose-async-hooks"
        ));
    }

    /** API frames default off, accept native flag aliases, and follow only actual metadata overrides. */
    #[test]
    fn experimental_api_stack_frames_use_actual_metadata() {
        let name = "experimental-stack-trace-frames";
        assert!(!boolean_flag("", name));
        assert!(boolean_flag(
            "// Flags: --experimental-stack-trace-frames",
            name
        ));
        assert!(boolean_flag(
            "// Flags: --experimental_stack_trace_frames=true",
            name
        ));
        assert!(!boolean_flag(
            "// Flags: --experimental-stack-trace-frames --noexperimental-stack-trace-frames",
            name
        ));
        assert!(boolean_flag(
            "// Flags: --no-experimental-stack-trace-frames --experimental-stack-trace-frames",
            name
        ));
        assert!(!boolean_flag(
            "const text='--experimental-stack-trace-frames'",
            name
        ));
    }

    /** The parser's result reaches actual native entry capture rather than a filename-specific runner shortcut. */
    #[cfg(feature = "conformance")]
    #[test]
    fn experimental_api_stack_frames_are_delivered_to_execution() {
        let checkout = Checkout::new();
        for (flags, expected) in [
            ("", false),
            ("// Flags: --experimental-stack-trace-frames\n", true),
            (
                "// Flags: --experimental-stack-trace-frames --no-experimental-stack-trace-frames\n",
                false,
            ),
        ] {
            let source = format!(
                "{flags}Error.prepareStackTrace=(_,s)=>s;const frames=Realm.eval(0,'new Error()').stack;const exposed=frames.some(f=>f.getFunction()===Realm.eval);if(exposed!=={expected})throw Error('wrong API frame visibility');"
            );
            assert_eq!(checkout.scripts(&source, "").execute(), Ok(()));
        }
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Actual --throws metadata accepts real throws and syntax errors, but never missing throws or assertions. */
    fn expected_exceptions_follow_the_declared_shell_option() {
        let checkout = Checkout::new();
        assert_eq!(
            checkout
                .scripts("// Flags: --throws\nthrow new Error('expected');", "")
                .execute(),
            Ok(())
        );
        assert_eq!(
            checkout
                .scripts("// Flags: --throws\nconst ;", "")
                .execute(),
            Ok(())
        );
        assert!(
            checkout
                .scripts("// Flags: --throws\n42;", "")
                .execute()
                .unwrap_err()
                .contains("Expected an uncaught")
        );
        assert!(checkout.scripts("// Flags: --throws\nclass MjsUnitAssertionError{get message(){return 'assertion'}};throw new MjsUnitAssertionError();", "").execute().is_err());
        assert!(checkout.scripts("// Flags: --throws\nnew Worker(()=>{}, {type:'function',flushDenormals:true});", "").execute().is_err());
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Binary fixtures retain invalid UTF-8 and each read returns an independent actual ArrayBuffer. */
    fn binary_file_alias_preserves_exact_bytes() {
        let checkout = Checkout::new();
        std::fs::write(checkout.root().join("binary.bin"), [0, 255, 128, 65]).unwrap();
        let scripts = checkout.scripts("const a=new Uint8Array(readbuffer('test/mjsunit/binary.bin'));const b=new Uint8Array(readbuffer('test/mjsunit/binary.bin'));a[0]=9;if(b.join(',')!=='0,255,128,65')throw Error('wrong binary clone');", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** The legacy load alias executes every file in order with persistent global lexical bindings. */
    fn load_alias_runs_ordered_global_scripts() {
        let checkout = Checkout::new();
        std::fs::write(checkout.root().join("first.js"), "const loadedFirst=40;").unwrap();
        std::fs::write(
            checkout.root().join("second.js"),
            "const loadedSecond=loadedFirst+2;",
        )
        .unwrap();
        let scripts = checkout.scripts("load();load('test/mjsunit/first.js','test/mjsunit/second.js');if(loadedSecond!==42)throw Error('wrong load scope');", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Native shell version reports this actual engine, and construction uses the real new.target prototype. */
    fn version_alias_reports_mars_and_constructs_ordinary_objects() {
        let checkout = Checkout::new();
        let scripts = checkout.scripts("if(!version().startsWith('Mars V9 '))throw Error('wrong engine identity');class V extends version{};const value=new V;if(!(value instanceof V)||Object.getPrototypeOf(value)!==V.prototype)throw Error('wrong native receiver');", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Wrapped file assertions still fail --throws, whereas an explicit ordinary catch remains normal JavaScript. */
    fn file_error_wrapping_preserves_assertion_and_native_failure_provenance() {
        let checkout = Checkout::new();
        std::fs::write(
            checkout.root().join("assert.js"),
            "throw new MjsUnitAssertionError();",
        )
        .unwrap();
        let prefix = "class MjsUnitAssertionError{get message(){return 'assertion'}};";
        assert!(
            checkout
                .scripts(
                    &format!("// Flags: --throws\n{prefix}load('test/mjsunit/assert.js')"),
                    ""
                )
                .execute()
                .is_err()
        );
        assert_eq!(checkout.scripts(&format!("{prefix}try{{load('test/mjsunit/assert.js')}}catch(error){{if(error.constructor!==Error)throw Error('wrong wrapper')}}"), "").execute(),Ok(()));
        std::fs::write(
            checkout.root().join("native.js"),
            "new Worker(()=>{}, {type:'function',flushDenormals:true});",
        )
        .unwrap();
        assert!(
            checkout
                .scripts(
                    "// Flags: --throws\ntry{load('test/mjsunit/native.js')}catch(error){}",
                    ""
                )
                .execute()
                .unwrap_err()
                .contains("unsupported_feature")
        );
        assert!(
            checkout
                .scripts("// Flags: --throws\nload('test/mjsunit/native.js')", "")
                .execute()
                .unwrap_err()
                .contains("unsupported_feature")
        );
        std::fs::write(checkout.root().join("exit.js"), "quit(0)").unwrap();
        assert_eq!(
            checkout
                .scripts(
                    "load('test/mjsunit/exit.js');throw Error('after fatal exit');",
                    ""
                )
                .execute(),
            Ok(())
        );
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Native bundle modules declared later are available to earlier ordered entrypoints and Scripts. */
    fn bundle_entrypoints_link_before_ordered_script_execution() {
        let checkout = Checkout::new();
        let scripts = checkout.scripts("// Flags: --bundle\n// JS_BUNDLE_MODULE_ENTRYPOINT\nimport {value} from 'later.mjs';globalThis.bundleValue=value;\n// JS_BUNDLE_MODULE:later.mjs\nexport const value=42;\n// JS_BUNDLE_SCRIPT\nif(bundleValue!==42)throw Error('wrong native execution order');", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[test]
    /** Script directives cannot spill across native bundle boundaries into the next Script. */
    fn bundle_script_directives_have_separate_global_scopes() {
        let checkout = Checkout::new();
        let scripts=checkout.scripts("// Flags: --bundle\n// JS_BUNDLE_SCRIPT\n'use strict';const bundleLexical=40;\n// JS_BUNDLE_SCRIPT\n(function(){if(this!==globalThis)throw Error('strict leaked')})();if(bundleLexical!==40)throw Error('global lexical missing');", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Bundle exports are real per-realm module bindings, visible through actual ShadowRealm importValue. */
    fn bundle_module_instances_stay_isolated_between_shadow_realms() {
        let checkout = Checkout::new();
        let scripts=checkout.scripts("// Flags: --bundle\n// JS_BUNDLE_MODULE:counter.mjs\nlet count=0;export function next(){return ++count}\n// JS_BUNDLE_MODULE_ENTRYPOINT\nconst a=new ShadowRealm(),b=new ShadowRealm();Promise.all([a.importValue('counter.mjs','next'),b.importValue('counter.mjs','next')]).then(([a,b])=>{if(a()!==1||a()!==2||b()!==1)throw Error('module state leaked')});", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[test]
    /** A dependency's top-level const is visible to the separately parsed test. */
    fn declared_files_run_as_global_scripts() {
        let checkout = Checkout::new();
        std::fs::write(
            checkout.root().join("helper.js"),
            "const prerequisite = 42;",
        )
        .unwrap();
        let scripts = checkout.scripts("// Files: test/mjsunit/helper.js\nif (prerequisite !== 42) throw new Error('missing lexical');", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[test]
    /** Harness concatenation must not disable the test's strict-mode directive. */
    fn test_strict_directives_are_preserved() {
        let checkout = Checkout::new();
        let scripts =
            checkout.scripts("'use strict'; strictUndeclared = 1;", "var harness = true;");
        assert!(scripts.execute().unwrap_err().contains("ReferenceError"));
    }

    #[test]
    /** A NO HARNESS request runs the actual test without prepending assertion source. */
    fn no_harness_is_honored() {
        let checkout = Checkout::new();
        assert_eq!(
            checkout
                .scripts(
                    "// NO HARNESS\nvar ran = true;",
                    "throw new Error('unwanted harness');"
                )
                .execute(),
            Ok(())
        );
    }

    #[test]
    /** Missing prerequisites remain explicit failures rather than disappearing cases. */
    fn missing_declared_files_fail() {
        let checkout = Checkout::new();
        let scripts = checkout.scripts("// Files: test/mjsunit/missing.js\n", "");
        assert!(
            scripts
                .execute()
                .unwrap_err()
                .contains("fixture test/mjsunit/missing.js")
        );
    }

    #[test]
    /** Canonicalization rejects traversals even when an outside fixture really exists. */
    fn declared_files_cannot_escape_the_checkout() {
        let checkout = Checkout::new();
        std::fs::write(checkout.0.join("outside.js"), "var secret = 1;").unwrap();
        let scripts = checkout.scripts("// Files: ../outside.js\n", "");
        assert!(
            scripts
                .execute()
                .unwrap_err()
                .contains("escapes upstream checkout")
        );
    }

    #[cfg(unix)]
    #[test]
    /** A fixture symlink cannot grant access outside the canonical checkout root. */
    fn declared_file_symlinks_cannot_escape_the_checkout() {
        let checkout = Checkout::new();
        std::fs::write(checkout.0.join("outside.js"), "var secret = 1;").unwrap();
        std::os::unix::fs::symlink(
            checkout.0.join("outside.js"),
            checkout.root().join("escape.js"),
        )
        .unwrap();
        let scripts = checkout.scripts("// Files: test/mjsunit/escape.js\n", "");
        assert!(
            scripts
                .execute()
                .unwrap_err()
                .contains("escapes upstream checkout")
        );
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Discovery follows real nested fixture references and executes their unchanged source. */
    fn fixture_discovery_loads_nested_scripts() {
        let checkout = Checkout::new();
        std::fs::write(
            checkout.root().join("outer.js"),
            "d8.file.execute('test/mjsunit/inner.js'); var outer = inner + 1;",
        )
        .unwrap();
        std::fs::write(checkout.root().join("inner.js"), "const inner = 41;").unwrap();
        let scripts = checkout.scripts(
            "d8.file.execute('test/mjsunit/outer.js'); if (outer !== 42) throw Error('bad order');",
            "",
        );
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[test]
    /** Real nested timers run to completion on V9's virtual clock. */
    fn nested_timers_are_executed() {
        let checkout = Checkout::new();
        let scripts = checkout.scripts("var count = 0; setTimeout(() => { count++; setTimeout(() => { if (count !== 1) throw Error('bad order'); }, 0); }, 0);", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** An assertion in a later task fails the case instead of passing at script-load time. */
    fn timer_exceptions_fail_the_case() {
        let checkout = Checkout::new();
        let scripts = checkout.scripts(
            "setTimeout(() => { throw new Error('late failure'); }, 0);",
            "",
        );
        assert!(scripts.execute().unwrap_err().contains("late failure"));
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Shell reporting cannot hide assertion failures, even if onerror attempts to suppress diagnostics. */
    fn microtask_assertions_fail_the_case_even_with_onerror() {
        let checkout = Checkout::new();
        let scripts = checkout.scripts(
            "class MjsUnitAssertionError extends Error{};onerror=()=>true;queueMicrotask(() => { throw new MjsUnitAssertionError('job failure'); });",
            "",
        );
        assert!(scripts.execute().unwrap_err().contains("job failure"));
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Only an actually unhandled rejection fails; same-turn catch handlers still work. */
    fn unhandled_rejections_fail_the_case() {
        let checkout = Checkout::new();
        assert!(
            checkout
                .scripts("Promise.reject(Error('unhandled'));", "")
                .execute()
                .unwrap_err()
                .contains("unhandled")
        );
        assert_eq!(
            checkout
                .scripts("Promise.reject(Error('handled')).catch(() => {});", "")
                .execute(),
            Ok(())
        );
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Console messages cannot impersonate the structured uncaught exception channel. */
    fn console_error_text_is_not_an_uncaught_exception() {
        let checkout = Checkout::new();
        assert_eq!(
            checkout
                .scripts("console.error('Uncaught (in promise) expected log');", "")
                .execute(),
            Ok(())
        );
    }

    #[test]
    /** An interval that never clears fails explicitly at a fixed callback allowance. */
    fn an_unending_event_loop_fails_explicitly() {
        let checkout = Checkout::new();
        assert!(
            checkout
                .scripts("setInterval(() => {}, 0);", "")
                .execute()
                .unwrap_err()
                .contains("host event loop callback limit exceeded")
        );
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** A fixture's lexical global survives state validation and is visible inside a later task. */
    fn fixture_globals_survive_timer_delivery() {
        let checkout = Checkout::new();
        std::fs::write(
            checkout.root().join("lexical.js"),
            "const fixtureLexical = 42;",
        )
        .unwrap();
        let scripts = checkout.scripts("d8.file.execute('test/mjsunit/lexical.js'); setTimeout(() => { if (fixtureLexical !== 42) throw Error('missing fixture lexical'); }, 0);", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[cfg(feature = "conformance")]
    #[test]
    /** Cyclic fixture discovery terminates while real recursive execute calls still run. */
    fn self_referenced_fixtures_are_loaded_once() {
        let checkout = Checkout::new();
        std::fs::write(checkout.root().join("self.js"), "var calls = (globalThis.calls || 0) + 1; if (calls < 2) d8.file.execute('test/mjsunit/self.js');").unwrap();
        let scripts = checkout.scripts("d8.file.execute('test/mjsunit/self.js'); if (calls !== 2) throw Error('missing recursive source');", "");
        assert_eq!(scripts.execute(), Ok(()));
    }

    #[test]
    /** The unsafe constructor flag defaults off and honors actual spellings and override order. */
    fn unsafe_constructor_flag_is_an_explicit_metadata_choice() {
        let name = "allow-unsafe-function-constructor";
        assert!(!boolean_flag("", name));
        assert!(boolean_flag(
            "// Flags: --allow_unsafe_function_constructor",
            name
        ));
        assert!(!boolean_flag(
            "// Flags: --allow-unsafe-function-constructor --noallow-unsafe-function-constructor",
            name
        ));
        assert!(boolean_flag(
            "// Flags: --no-allow-unsafe-function-constructor --allow-unsafe-function-constructor=true",
            name
        ));
        assert!(!boolean_flag(
            "const text='--allow-unsafe-function-constructor'",
            name
        ));
    }

    #[cfg(not(feature = "conformance"))]
    #[test]
    /** Default browser builds cannot access the upstream-only file and collection host. */
    fn production_build_omits_the_test_host() {
        assert_eq!(
            crate::common::eval("const result = typeof d8 + ':' + typeof gc;", "result"),
            "undefined:undefined"
        );
    }
}
