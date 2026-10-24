/**
 * The navigation position the Page domain's history methods need: how many entries the tab's
 * history holds and which one is current. The entries' URLs stay inside the runtime (they may
 * be sensitive), so the front door reports positions and the current page only.
 */
use super::super::supervisor::Runtime;
use crate::protocol::{EngineResult, parameters};
use serde_json::{Value, json};

impl Runtime {
    /** page.history: entry count and the current index. */
    pub(in crate::runtime) fn history_position(&mut self, params: Value) -> EngineResult<Value> {
        let _: super::super::page::Empty = parameters(params)?;
        Ok(json!({"length":self.history.len(),"index":self.history_cursor.unwrap_or(0)}))
    }
}
