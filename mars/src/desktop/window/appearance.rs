/** Native appearance transitions join the existing bounded runtime and UI acknowledgment path. */
use super::*;
use crate::{document::Appearance as Scheme, protocol::parameters, runtime::authority};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
};
use objc2_foundation::NSArray;
use serde::Deserialize;

/** The previous OS override stays UI-owned until its exact runtime update is displayed. */
pub(super) struct PendingAppearance {
    /** A matching response cannot acknowledge another queued operation. */
    id: String,
    /** Takeover invalidates both queued rendering and the eventual UI response. */
    permit: Permit,
    /** Failure restores the window's prior explicit or inherited preference. */
    previous: Option<Retained<NSAppearance>>,
    /** Successful acknowledgment must describe the context that actually committed. */
    context: RenderContext,
    /** A revoked in-flight operation restores its OS override exactly once. */
    restored: bool,
}

/** OS preference changes remain an explicitly authorized native operation. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppearanceRequest {
    /** Only the currently visible authorized tab may change the shared native window. */
    tab_id: String,
    /** System removes a window override and observes its effective inherited appearance. */
    appearance: String,
    /** Client preconditions cannot be replaced with a newer native grant. */
    control_epoch: u64,
}

impl Window {
    /** Effective appearance and post-banner scroll size are sampled together on the UI thread. */
    pub(super) fn render_context(&self) -> RenderContext {
        let zoom = self.mars.scroll.magnification().max(0.01);
        let size = self.mars.scroll.contentSize();
        let size = objc2_foundation::NSSize::new(
            (size.width / zoom).clamp(human::PAGE_WIDTH_MIN, human::PAGE_WIDTH_MAX),
            (size.height / zoom).max(crate::constants::MIN_VIEWPORT_HEIGHT),
        );
        let names =
            unsafe { NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]) };
        let matched = self
            .window
            .effectiveAppearance()
            .bestMatchFromAppearancesWithNames(&names);
        let dark = matched
            .as_deref()
            .is_some_and(|name| unsafe { name == NSAppearanceNameDarkAqua });
        RenderContext {
            width: size.width,
            height: size.height,
            appearance: if dark { Scheme::Dark } else { Scheme::Light },
        }
    }
    /** Validate the exact visible agent grant without holding authority during OS work. */
    fn appearance_permit(&self, permit: Permit) -> EngineResult<()> {
        if permit.owner != Owner::Agent
            || !self.broker.connected.load(Ordering::Acquire)
            || self.broker.cancel.load(Ordering::Acquire)
        {
            return Err(authority::denied());
        }
        authority::lock(&self.broker.authority)?
            .validate_tab(permit, crate::runtime::tab_key(&self.selected_tab_id())?)
    }
    /** No success response is produced until the resulting immutable snapshot is consumed. */
    pub(super) fn begin_appearance(
        &mut self,
        request: Request,
        permit: Permit,
    ) -> EngineResult<()> {
        request.validate()?;
        let appearance: AppearanceRequest = parameters(request.params)?;
        if self.pending_appearance.is_some() {
            return Err(EngineError::invalid(
                "A native appearance update is already pending.",
            ));
        }
        if appearance.tab_id != self.selected_tab_id() || appearance.control_epoch != permit.epoch {
            return Err(authority::denied());
        }
        self.appearance_permit(permit)?;
        let value = requested_appearance(&appearance.appearance)?;
        let previous = self.window.appearance();
        self.window.setAppearance(value.as_deref());
        self.layout();
        let context = self.render_context();
        let admitted = context.validate().and_then(|()| {
            self.broker
                .commands
                .try_send(Command::Appearance {
                    context,
                    permit,
                    id: request.id.clone(),
                    response: json!({"appearance":appearance.appearance}),
                })
                .map_err(|_| {
                    EngineError::new(
                        "service_unavailable",
                        "Native appearance update could not be queued.",
                        "Inspect the current window before trying again.",
                    )
                })
        });
        if let Err(error) = admitted {
            self.window.setAppearance(previous.as_deref());
            self.canvas.setNeedsDisplay(true);
            return Err(error);
        }
        self.pending_appearance = Some(PendingAppearance {
            id: request.id,
            permit,
            previous,
            context,
            restored: false,
        });
        self.busy = true;
        self.agent_busy = true;
        Ok(())
    }
    /** Takeover restores the OS override while the worker completes its bounded cancellation. */
    pub(super) fn cancel_stale_appearance(&mut self) {
        let stale = self.pending_appearance.as_ref().is_some_and(|pending| {
            !pending.restored && self.appearance_permit(pending.permit).is_err()
        });
        if stale && let Some(pending) = &mut self.pending_appearance {
            self.window.setAppearance(pending.previous.as_deref());
            self.canvas.setNeedsDisplay(true);
            pending.restored = true;
        }
    }
    /** Only the matching consumed update can complete an explicit appearance transition. */
    pub(super) fn finish_appearance(&mut self, id: Option<&str>, result: &mut EngineResult<Value>) {
        if self
            .pending_appearance
            .as_ref()
            .is_none_or(|pending| Some(pending.id.as_str()) != id)
        {
            return;
        }
        let pending = self
            .pending_appearance
            .take()
            .expect("matched appearance transition");
        if result.is_ok() {
            *result = self.appearance_permit(pending.permit).and_then(|()| {
                if self
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.render_context == pending.context)
                {
                    Ok(result
                        .as_ref()
                        .expect("successful appearance result")
                        .clone())
                } else {
                    Err(EngineError::invalid(
                        "Native appearance context did not commit.",
                    ))
                }
            });
        }
        if result.is_err() {
            self.window.setAppearance(pending.previous.as_deref());
            self.canvas.setNeedsDisplay(true);
        } else {
            self.last_context = Some(pending.context);
        }
        self.busy = false;
    }
}

/** Resolve supported OS overrides explicitly rather than treating an unavailable name as system. */
fn requested_appearance(name: &str) -> EngineResult<Option<Retained<NSAppearance>>> {
    let value = match name {
        "light" => unsafe { NSAppearance::appearanceNamed(NSAppearanceNameAqua) },
        "dark" => unsafe { NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) },
        "system" => return Ok(None),
        _ => {
            return Err(EngineError::invalid(
                "Expected light, dark or system appearance.",
            ));
        }
    };
    value
        .map(Some)
        .ok_or_else(|| EngineError::unsupported("The requested native appearance is unavailable."))
}
