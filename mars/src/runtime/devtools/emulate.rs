/**
 * Emulation for the Emulation and Network domains: the viewport and colour scheme (shared by
 * every tab, as Mars has one display), and the browser facts the page's scripts read (user
 * agent, platform, timezone, languages). Overrides reach scripts from the next step on; what
 * Relay sends on the wire is unchanged (see docs/cdp.md).
 */
use super::super::supervisor::Runtime;
use crate::document::{Appearance, RenderContext};
use crate::protocol::{EngineResult, parameters};
use serde::Deserialize;
use serde_json::{Value, json};

/** The viewport in logical points. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Viewport {
    width: f64,
    height: f64,
}

/** page.emulate: every field is optional and independent. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Emulate {
    #[serde(default)]
    viewport: Option<Viewport>,
    #[serde(default)]
    appearance: Option<Appearance>,
    #[serde(default)]
    user_agent: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    timezone_offset_minutes: Option<f64>,
    #[serde(default)]
    languages: Option<Vec<String>>,
}

impl Runtime {
    /** page.emulate: apply the given overrides and report the state they left. */
    pub(in crate::runtime) fn emulate(&mut self, params: Value) -> EngineResult<Value> {
        let input: Emulate = parameters(params)?;
        if input.viewport.is_some() || input.appearance.is_some() {
            let context = RenderContext {
                width: input
                    .viewport
                    .as_ref()
                    .map_or(self.render_context.width, |viewport| viewport.width),
                height: input
                    .viewport
                    .as_ref()
                    .map_or(self.render_context.height, |viewport| viewport.height),
                appearance: input.appearance.unwrap_or(self.render_context.appearance),
            };
            self.set_render_context(context)?;
        }
        if let Some(languages) = input.languages {
            self.languages = super::super::languages::usable(languages);
        }
        self.emulate_environment(
            input.user_agent,
            input.platform,
            input.timezone_offset_minutes,
        )?;
        Ok(json!({
            "viewport": {"width": self.render_context.width, "height": self.render_context.height},
            "appearance": self.render_context.appearance,
            "languages": self.languages,
        }))
    }

    /** Patch the committed page's script environment; a page without a document has none. */
    fn emulate_environment(
        &mut self,
        user_agent: Option<String>,
        platform: Option<String>,
        timezone: Option<f64>,
    ) -> EngineResult<()> {
        if user_agent.is_none() && platform.is_none() && timezone.is_none() {
            return Ok(());
        }
        let Some(mut page) = self.page.clone() else {
            return Ok(());
        };
        let environment = &mut page.scripting.environment;
        if let Some(user_agent) = user_agent {
            environment.user_agent = user_agent;
        }
        if let Some(platform) = platform {
            environment.platform = platform;
        }
        if let Some(offset) = timezone.filter(|offset| offset.is_finite()) {
            environment.timezone_offset_minutes = offset;
        }
        self.commit_quietly(page)
    }
}
