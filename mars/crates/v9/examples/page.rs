/**
 * Measurement harness for a whole page lifecycle: load scripts, fire DOMContentLoaded and load,
 * then advance the virtual clock in steps, printing console output and effects. `--scale N`
 * multiplies V9's heap bounds; `--extended` enables the extended change log. Requires `measure`.
 */
use mars_v9::{
    DocumentHost, DomLevel, PageEnvironment, PageEvent, PageInput, SimpleFragmentParser,
    StorageState, document::ScriptDocument,
};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut scale = 1;
    let mut level = DomLevel::Legacy;
    let mut click: Option<String> = None;
    loop {
        match args.first().map(String::as_str) {
            Some("--scale") => {
                scale = args[1].parse().unwrap_or(1);
                args.drain(0..2);
            }
            Some("--extended") => {
                level = DomLevel::Extended;
                args.remove(0);
            }
            Some("--click") => {
                click = Some(args[1].clone());
                args.drain(0..2);
            }
            _ => break,
        }
    }
    let sources: Vec<String> = args
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap_or_default())
        .collect();
    let mut environment = PageEnvironment {
        url: std::env::var("PAGE_URL").unwrap_or_else(|_| "https://example.com/".into()),
        time_origin_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0),
        dom_level: level,
        ..PageEnvironment::default()
    };
    let host = DocumentHost {
        languages: &[],
        fragments: &SimpleFragmentParser,
    };
    let mut document_now = ScriptDocument {
        identity: "page".into(),
        charset: "UTF-8".into(),
        nodes: Vec::new(),
    };
    let report = |label: &str, execution: &mars_v9::Execution| {
        println!(
            "-- {label}: {} changes, {} script changes, next timer {:?}",
            execution.changes.len(),
            execution.script_changes.len(),
            execution.effects.next_timer_ms
        );
        for entry in &execution.effects.console {
            println!("[{}] {}", entry.level, entry.text);
        }
        if execution.effects.navigation.is_some() || !execution.effects.cookies.is_empty() {
            println!("navigation {:?} cookies {:?}", execution.effects.navigation, execution.effects.cookies);
        }
        for failure in &execution.failed {
            println!("FAILED script {}: {}", failure.index, failure.error.message);
        }
    };
    let loaded = match mars_v9::measure::load_page_scaled(
        input(&sources, &document_now, &host, &environment),
        scale,
    ) {
        Ok(execution) => execution,
        Err(error) => return println!("load failed: {}", error.message),
    };
    report("load", &loaded);
    let mut node_text = environment.node_text.clone();
    if let Err(error) = mars_v9::replay(
        &mut document_now,
        &loaded,
        &SimpleFragmentParser,
        &mut node_text,
    ) {
        return println!("replay failed: {}", error.message);
    }
    environment.node_text = node_text;
    let mut state = loaded.state;
    let mut steps: Vec<Option<PageEvent>> =
        vec![Some(PageEvent::DomContentLoaded), Some(PageEvent::Load)];
    steps.extend(std::iter::repeat_n(None, 20));
    let mut now = 0.0;
    for step in steps {
        let result = match step.clone() {
            Some(event) => mars_v9::measure::page_event_scaled(
                input(&sources, &document_now, &host, &environment),
                state.clone(),
                event,
                scale,
            ),
            None => {
                now += 25.0;
                mars_v9::measure::run_timers_scaled(
                    input(&sources, &document_now, &host, &environment),
                    state.clone(),
                    now,
                    100,
                    scale,
                )
            }
        };
        let execution = match result {
            Ok(execution) => execution,
            Err(error) => return println!("step failed: {}", error.message),
        };
        if step.is_some() || !execution.effects.console.is_empty() || !execution.failed.is_empty() {
            report(
                &step.clone().map_or_else(|| format!("timers @{now}"), |event| format!("{event:?}")),
                &execution,
            );
        }
        let mut node_text = environment.node_text.clone();
        if let Err(error) = mars_v9::replay(
            &mut document_now,
            &execution,
            &SimpleFragmentParser,
            &mut node_text,
        ) {
            return println!("replay failed: {}", error.message);
        }
        environment.node_text = node_text;
        let done = step.is_none() && execution.effects.next_timer_ms.is_none();
        state = execution.state;
        if done {
            break;
        }
    }
    println!("final projection: {} nodes", document_now.nodes.len());
    if let Some(tag) = click {
        let Some(target) = document_now
            .nodes
            .iter()
            .find(|node| node.tag == tag)
            .map(|node| node.local_id)
        else {
            return println!("no <{tag}> to click");
        };
        let clicked = match mars_v9::measure::click_page_scaled(
            input(&sources, &document_now, &host, &environment),
            state.clone(),
            target,
            scale,
        ) {
            Ok(execution) => execution,
            Err(error) => return println!("click failed: {}", error.message),
        };
        report("click", &clicked);
        let mut node_text = environment.node_text.clone();
        if let Err(error) = mars_v9::replay(
            &mut document_now,
            &clicked,
            &SimpleFragmentParser,
            &mut node_text,
        ) {
            return println!("replay failed: {}", error.message);
        }
        environment.node_text = node_text;
        state = clicked.state;
        for _ in 0..5 {
            now += 25.0;
            match mars_v9::measure::run_timers_scaled(
                input(&sources, &document_now, &host, &environment),
                state.clone(),
                now,
                100,
                scale,
            ) {
                Ok(execution) => {
                    report(&format!("timers @{now}"), &execution);
                    let mut node_text = environment.node_text.clone();
                    if let Err(error) = mars_v9::replay(
                        &mut document_now,
                        &execution,
                        &SimpleFragmentParser,
                        &mut node_text,
                    ) {
                        return println!("replay failed: {}", error.message);
                    }
                    environment.node_text = node_text;
                    state = execution.state;
                }
                Err(error) => return println!("timers failed: {}", error.message),
            }
        }
        let texts: Vec<&String> = environment.node_text.values().collect();
        println!("texts after click: {texts:?}");
    }
}

/** One request's input. */
fn input<'a>(
    sources: &'a [String],
    document: &ScriptDocument,
    host: &'a DocumentHost<'a>,
    environment: &'a PageEnvironment,
) -> PageInput<'a> {
    PageInput {
        sources,
        source_keys: &[],
        document: document.clone(),
        storage: StorageState::default(),
        host,
        environment,
        modules: mars_v9::ModuleInput::NONE,
    }
}
