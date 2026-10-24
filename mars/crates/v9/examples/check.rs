/** Parse classic script files without running them, or with `--run` load them as one page. */
use mars_v9::{DocumentHost, StorageState, document::ScriptDocument};

fn main() {
    let mut paths = std::env::args().skip(1).collect::<Vec<_>>();
    let run = paths.first().is_some_and(|flag| flag == "--run");
    if run {
        paths.remove(0);
    }
    let sources = paths
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap_or_default())
        .collect::<Vec<_>>();
    if run {
        let document = ScriptDocument {
            identity: "example".into(),
            nodes: Vec::new(),
        };
        let started = std::time::Instant::now();
        let result = mars_v9::load_document_scripts_isolated(
            &sources,
            document,
            StorageState::default(),
            &DocumentHost::default(),
        );
        println!("loaded in {:?}", started.elapsed());
        match result {
            Ok(execution) => {
                for failure in execution.failed {
                    println!("{}: {}", paths[failure.index], failure.error.message);
                }
            }
            Err(error) => println!("whole load failed: {}", error.message),
        }
        return;
    }
    for (path, source) in paths.iter().zip(&sources) {
        let report = match mars_v9::check(source) {
            Ok(()) => "parsed".to_string(),
            Err(error) => format!("{}: {}", error.code, error.message),
        };
        println!("{path}: {report}");
    }
}
