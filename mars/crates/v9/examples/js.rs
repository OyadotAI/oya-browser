/**
 * Measurement harness: load script files (or `-e source`) as one page and print console output,
 * failures, unsupported APIs and the committed state's size. `--scale N` multiplies V9's heap
 * bounds to show what a script would need. Requires the `measure` feature.
 */
use mars_v9::{
    DocumentHost, PageEnvironment, PageInput, SimpleFragmentParser, StorageState,
    document::ScriptDocument,
};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut scale = 1;
    if args.first().is_some_and(|a| a == "--scale") {
        scale = args[1].parse().unwrap_or(1);
        args.drain(0..2);
    }
    let sources: Vec<String> = if args.first().is_some_and(|a| a == "-e") {
        vec![args[1].clone()]
    } else {
        args.iter()
            .map(|path| std::fs::read_to_string(path).unwrap_or_default())
            .collect()
    };
    let environment = PageEnvironment {
        url: "https://example.com/page".into(),
        ..PageEnvironment::default()
    };
    let host = DocumentHost {
        languages: &[],
        fragments: &SimpleFragmentParser,
    };
    let started = std::time::Instant::now();
    let input = PageInput {
        sources: &sources,
        document: ScriptDocument {
            identity: "page".into(),
            nodes: Vec::new(),
        },
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    };
    let result = mars_v9::measure::load_page_scaled(input, scale);
    println!("ran in {:?}", started.elapsed());
    match result {
        Ok(execution) => {
            for entry in &execution.effects.console {
                println!("[{}] {}", entry.level, entry.text);
            }
            for failure in &execution.failed {
                println!("FAILED script {}: {}", failure.index, failure.error.message);
            }
            if !execution.effects.unsupported.is_empty() {
                println!("unsupported: {:?}", execution.effects.unsupported);
            }
            if let Some((objects, functions, properties, units, environments, bindings, bytes)) =
                mars_v9::measure::last_counts()
            {
                println!(
                    "live: objects {objects} functions {functions} properties {properties} units {units} environments {environments} bindings {bindings} state bytes {bytes}"
                );
            }
            println!(
                "requests {} timers next {:?}",
                execution.effects.requests.len(),
                execution.effects.next_timer_ms
            );
        }
        Err(error) => println!("whole load failed: {} {}", error.code, error.message),
    }
}
