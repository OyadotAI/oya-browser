/** Explicit native observation and screenshot APIs run on the OS owning thread. */
use super::*;
use crate::{protocol::parameters, runtime::authority};
use serde::Deserialize;
/** Native OS observations retain strict parameter schemas like the engine protocol. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
/** Captures explicitly identify the page and exact painted presentation. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Shot {
    tab_id: String,
    page_id: String,
    expected_revision: u64,
    layout_generation: u64,
}
/** Window size is a startup-granted agent mutation, never a page action. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resize {
    tab_id: String,
    width: f64,
    height: f64,
    control_epoch: u64,
}

impl Window {
    /** The owning UI thread implements only a small explicit OS command table. */
    pub(super) fn window_request(
        &mut self,
        mut request: Request,
        permit: Permit,
    ) -> EngineResult<Value> {
        request.validate()?;
        // The attach front door stamps its lane on every request; window operations have no lane
        // parameter, and would otherwise refuse every attached agent's observation.
        if let Some(params) = request.params.as_object_mut() {
            params.remove("lane");
        }
        match request.op.as_str() {
            "window.inspect" => {
                let _: Empty = parameters(request.params)?;
                let size = self.mars.root.bounds().size;
                let tab_id = self.selected_tab_id();
                let owner = if self.broker.permit(Owner::Human, &tab_id).is_ok() {
                    Owner::Human
                } else {
                    Owner::Agent
                };
                let observation = json!({"title":self.window.title().to_string(),"width":size.width,"height":size.height,"page":self.snapshot.as_ref().map(|snapshot|snapshot.inspection.clone()),"error":self.error,"control":owner,"tab_id":tab_id});
                #[cfg(feature = "test-support")]
                let observation = {
                    let mut observation = observation;
                    observation["navigation"] = json!({"busy":self.busy,"activity":self.activity,"requested":self.navigation.requested,"committed":self.mars.committed_url(),"address":self.mars.address.stringValue().to_string(),"stop_visible":self.mars.reload.tag()==mars::STOP,"stopping":self.navigation.stopping,"pending":self.navigation.pending});
                    observation["native_frames"] = self.canvas.native_frames();
                    observation["native_banner"] = self.mars.banner_observation();
                    observation["render_context"] = self
                        .snapshot
                        .as_ref()
                        .map(|snapshot| snapshot.render_context)
                        .map_or(Value::Null, |context| json!(context));
                    observation["native_validation"] =
                        self.canvas.validation_regions(&self.mars.root);
                    observation
                };
                Ok(observation)
            }
            "page.screenshot" => {
                let shot: Shot = parameters(request.params)?;
                let inspection = self
                    .snapshot
                    .as_ref()
                    .map(|snapshot| &snapshot.inspection)
                    .ok_or_else(|| EngineError::invalid("No visible page."))?;
                // The frame on screen is what a screenshot shows. A page that kept changing since the
                // caller looked is still that page, so the capture is of its presented frame and the
                // reply says which revision that was; only a different page is refused.
                if inspection["tab_id"] != shot.tab_id || inspection["page_id"] != shot.page_id {
                    return Err(EngineError::invalid(
                        "Screenshot targets a page that is no longer visible.",
                    ));
                }
                if !self.capture.granted() {
                    // Without the operator's --capture-dir grant, pixels go only to an agent the
                    // person handed this tab to, at the tab's current control epoch.
                    authority::lock(&self.broker.authority)?
                        .validate_tab(permit, crate::runtime::tab_key(&shot.tab_id)?)?;
                }
                let timed = std::time::Instant::now();
                let mut captured = self.capture.save(&self.mars.root, inspection)?;
                crate::runtime::perf::record("ui.capture", timed, String::new);
                captured["requested_revision"] = json!(shot.expected_revision);
                captured["requested_layout_generation"] = json!(shot.layout_generation);
                Ok(captured)
            }
            "window.resize" => {
                let size: Resize = parameters(request.params)?;
                if !size.width.is_finite()
                    || !size.height.is_finite()
                    || !(640.0..=2400.0).contains(&size.width)
                    || !(480.0..=1600.0).contains(&size.height)
                {
                    return Err(EngineError::invalid(
                        "Window size is outside supported bounds.",
                    ));
                }
                let state = authority::lock(&self.broker.authority)?;
                state.validate_tab(permit, crate::runtime::tab_key(&size.tab_id)?)?;
                if size.control_epoch != permit.epoch {
                    return Err(authority::denied());
                }
                drop(state);
                self.window
                    .setContentSize(NSSize::new(size.width, size.height));
                Ok(json!({"width":size.width,"height":size.height,"layout_pending":true}))
            }
            #[cfg(feature = "test-support")]
            "window.human" => self.human_request(request.params),
            _ => Err(EngineError::unsupported("Unknown native window operation.")),
        }
    }
}
