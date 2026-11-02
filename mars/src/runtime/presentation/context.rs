/** Presentation candidates retain edits and CSS while exact captured authority guards their publication. */
use super::super::{
    authority::{self, Permit},
    page::Page,
    supervisor::{Runtime, tab_key},
};
use crate::{
    document::RenderContext,
    protocol::{EngineError, EngineResult},
};
use std::sync::atomic::Ordering;

impl Runtime {
    /// OS resize and preference observations may reflow either owner's page without granting mutation.
    pub fn set_render_context(&mut self, context: RenderContext) -> EngineResult<()> {
        let permit = self.presentation_permit(None)?;
        self.change_context(context, permit)
    }
    /// Explicit native appearance commands retain their admitted tab owner and epoch through reflow.
    pub fn set_render_context_as(
        &mut self,
        context: RenderContext,
        permit: Permit,
    ) -> EngineResult<()> {
        let permit = self.presentation_permit(Some(permit))?;
        self.change_context(context, permit)
    }
    /** Observation captures an existing grant and never manufactures a human or agent owner. */
    pub(in crate::runtime) fn presentation_permit(
        &self,
        supplied: Option<Permit>,
    ) -> EngineResult<Option<Permit>> {
        let Some(shared) = &self.authority else {
            return Ok(None);
        };
        let state = authority::lock(shared)?;
        let tab = tab_key(&self.active_tab_id)?;
        if let Some(permit) = supplied {
            state.validate_tab(permit, tab)?;
            return Ok(Some(permit));
        }
        let (owner, _) = state.tab_status(tab).ok_or_else(authority::denied)?;
        state.observe_for(owner, tab).map(Some)
    }
    /** All expensive styling, measuring and encoding finishes before the current context changes. */
    fn change_context(
        &mut self,
        context: RenderContext,
        permit: Option<Permit>,
    ) -> EngineResult<()> {
        context.validate()?;
        self.healthy()?;
        if self.render_context == context {
            return Ok(());
        }
        let mut candidate = self.page.clone();
        if let Some(page) = candidate.as_mut() {
            // The generation advances first, so the presentation's kept inspection names it.
            page.layout_generation = page.layout_generation.saturating_add(1);
            self.prepare_presentation(page, context)?;
        }
        self.commit_context(context, candidate, permit)
    }
    /** A takeover or disconnect racing with reflow invalidates only the unpublished candidate. */
    fn commit_context(
        &mut self,
        context: RenderContext,
        page: Option<Page>,
        permit: Option<Permit>,
    ) -> EngineResult<()> {
        let shared = self.authority.clone();
        let _guard = if let Some(shared) = &shared {
            let guard = authority::lock(shared)?;
            guard.validate_tab(
                permit.ok_or_else(authority::denied)?,
                tab_key(&self.active_tab_id)?,
            )?;
            Some(guard)
        } else {
            None
        };
        if self.cancel.load(Ordering::Acquire) {
            return Err(EngineError::service("runtime", false));
        }
        self.page = page;
        self.render_context = context;
        self.clamp_scroll();
        Ok(())
    }
}
