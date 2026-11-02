/** WPT's Escape adapter queues real native input; only the runtime's completed action settles the page promise. */
use super::{Value, agent_call, json};
use mars::{
    protocol::{EngineError, EngineResult},
    runtime::Runtime,
};

/** The local vendor binding deliberately implements only a connected focused Escape target. */
pub(super) const VENDOR: &str = r#"(function(){
  if (!self.test_driver_internal) return;
  const queue=[];let sequence=0;
  self.__mars_wpt_driver={
    pending(){const r=queue[0];return r?{id:r.id,key:'Escape'}:null},
    target(){const r=queue[0];return r?r.element:null},
    complete(id,error){
      const r=queue[0];if(!r||r.id!==id)throw new Error('Stale native input request');
      queue.shift();if(error)r.reject(new Error(error));else r.resolve();
    }
  };
  test_driver_internal.in_automation=true;
  test_driver_internal.send_keys=function(element,keys){
    if(keys!=='\uE00C')return Promise.reject(new Error('Native WPT input supports Escape only'));
    if(!element||!element.isConnected||element!==document.activeElement)
      return Promise.reject(new Error('Native WPT input requires the connected focused element'));
    if(queue.length>=64||sequence>=4294967295)return Promise.reject(new Error('Native WPT input queue is full'));
    return new Promise((resolve,reject)=>queue.push({id:++sequence,element,resolve,reject}));
  };
})();"#;

/** Submit one pending request through the owned front door; no page promise settles before completion. */
pub(super) fn pump(runtime: &mut Runtime) -> EngineResult<bool> {
    let pending = evaluate(
        runtime,
        "self.__mars_wpt_driver ? __mars_wpt_driver.pending() : null",
        true,
    )?;
    let request = &pending["result"]["value"];
    let Some(id) = request["id"].as_u64() else {
        return Ok(false);
    };
    let target = evaluate(runtime, "__mars_wpt_driver.target()", false)?;
    let page = target["page_id"].as_str().ok_or_else(invalid_target)?;
    let node = target["result"]["marsNode"]
        .as_u64()
        .ok_or_else(invalid_target)?;
    let input = agent_call(
        runtime,
        "page.press",
        json!({"page_id":page,"expected_revision":target["revision"],"node_id":format!("{page}:node-{node}"),"key":"Escape"}),
    );
    complete(runtime, id, page, input)?;
    let _ = agent_call(
        runtime,
        "page.devtools",
        json!({"request":{"kind":"release_group","group":"wpt-native-driver"}}),
    );
    Ok(true)
}

/** Input completion is acknowledged only in the exact page generation that submitted it. */
fn complete(
    runtime: &mut Runtime,
    id: u64,
    page: &str,
    input: EngineResult<Value>,
) -> EngineResult<()> {
    let inspection = agent_call(runtime, "page.inspect", json!({}))?;
    if inspection["page_id"] != page {
        return Err(invalid_target());
    }
    let error = match input {
        Ok(result) if result["page_id"] == page => Value::Null,
        Ok(_) => return Err(invalid_target()),
        Err(error) => json!(error.message),
    };
    evaluate(
        runtime,
        &format!("__mars_wpt_driver.complete({id},{error})"),
        true,
    )?;
    Ok(())
}

/** The native evaluator transports queue metadata and element identities, never a replacement input algorithm. */
fn evaluate(runtime: &mut Runtime, expression: &str, by_value: bool) -> EngineResult<Value> {
    let result = agent_call(
        runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":expression,"options":{"return_by_value":by_value,"group":"wpt-native-driver"}}}),
    )?;
    if result
        .get("exceptionDetails")
        .is_some_and(|error| !error.is_null())
    {
        return Err(EngineError::invalid(
            "The native WPT input adapter could not inspect its pending request.",
        ));
    }
    Ok(result)
}

/** Detached or navigated targets cannot grant authority to another page's input queue. */
fn invalid_target() -> EngineError {
    EngineError::new(
        "invalid_target",
        "The WPT input target no longer belongs to the active page.",
        "Submit input only to the connected focused element of the current test page.",
    )
}
