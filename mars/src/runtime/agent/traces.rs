/**
 * Console and network views for agents, read from what the committed page recorded. They carry
 * no headers, cookies, request bodies or field values, and a sensitive URL loses its query.
 */
use super::super::observe::cap;
use super::super::page::{Empty, safe_url};
use super::super::supervisor::Runtime;
use crate::protocol::{EngineResult, parameters};
use serde_json::{Value, json};

/// Entries one trace call returns; the page's own diagnostics are already bounded too.
const TRACE_ENTRIES: usize = 50;

impl Runtime {
    /**
     * page.console: the engine's messages about this page (skipped scripts, removed handlers,
     * refused resources), then what the page's own scripts logged, newest last. Page code never
     * receives secret field values, so its output carries none.
     */
    pub(super) fn console(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let page = self.current()?;
        let mut entries = page
            .document
            .diagnostics
            .iter()
            .take(TRACE_ENTRIES)
            .map(|message| json!({"level":"warning","source":"engine","text":cap(message)}))
            .collect::<Vec<_>>();
        let logged = &page.scripting.console;
        let skip = logged.len().saturating_sub(TRACE_ENTRIES);
        entries.extend(
            logged
                .iter()
                .skip(skip)
                .map(|entry| json!({"level":entry.level,"source":"page","text":cap(&entry.text)})),
        );
        Ok(
            json!({"page_id":page.id(),"revision":page.revision,"entries":entries,"degraded":page.degraded,"page_console":"available","page_dropped":page.scripting.console_dropped + skip}),
        )
    }

    /**
     * page.network: the loads behind the committed page. The document is listed with its method
     * and status; images with type and size; scripts and stylesheets by count.
     */
    pub(super) fn network(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let page = self.current()?;
        let method = if page.replayable { "GET" } else { "POST" };
        let mut entries = vec![
            json!({"kind":"document","method":method,"url":safe_url(&page.url,page.sensitive),"status":page.status}),
        ];
        entries.extend(
            page.images
                .values()
                .take(TRACE_ENTRIES - 1)
                .map(|image| json!({"kind":"image","mime":image.mime,"bytes":image.bytes.len()})),
        );
        Ok(
            json!({"page_id":page.id(),"revision":page.revision,"entries":entries,"images":page.images.len(),"scripts":page.scripts.len(),"stylesheets":page.resolved_stylesheets.len(),"degraded":page.degraded}),
        )
    }
}
