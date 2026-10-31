/** Later native tasks deliver committed broadcast messages under each recipient tab's current authority. */
use super::*;
use crate::v9::PageEvent;
use std::time::Instant;

impl Runtime {
    /** One task per idle slice prevents cross-tab message loops from monopolizing user input. */
    pub(in crate::runtime) fn run_broadcast(
        &mut self,
        yield_now: &mut dyn FnMut() -> bool,
    ) -> bool {
        let letter = self.broadcast.next();
        let tab = letter
            .as_ref()
            .map(|letter| letter.tab.clone())
            .or_else(|| self.broadcast_background_tab(Instant::now()));
        let Some(tab) = tab else {
            return false;
        };
        // Asked only once there is work, so an idle broadcast never costs the page its step.
        if yield_now() {
            return false;
        }
        if self.healthy().is_err() {
            return false;
        }
        let visible = tab == self.selected_tab_id();
        let result = self.run_in_tab(&tab, |runtime| {
            let permit = runtime.presentation_permit(None)?;
            let previous = std::mem::replace(&mut runtime.permit, permit);
            let result = match letter {
                Some(letter) => {
                    let serial = letter.serial;
                    match runtime.broadcast_task(letter) {
                        Err(error) if error.code == super::super::service::STATE_MISSING => {
                            Ok(runtime.reload_lost_page(&error))
                        }
                        Err(error) if !super::super::worker_loading::fatal(&error) => {
                            runtime.failed_broadcast(serial, &error)
                        }
                        result => result,
                    }
                }
                None => runtime.tick(Instant::now(), yield_now),
            };
            runtime.permit = previous;
            result
        });
        visible && result.unwrap_or(false)
    }

    /** Terminal callback failures consume one task only after reporting on the unchanged committed page. */
    fn failed_broadcast(&mut self, serial: u64, error: &EngineError) -> EngineResult<bool> {
        let mut page = self.current()?.clone();
        super::super::scripting::stop_scripts(&mut page, error);
        page.scripting.broadcast.acknowledgments.push(serial);
        self.commit_mutation(page)?;
        Ok(true)
    }

    /** Ack and callback effects publish together, so aborted candidates cannot lose or send messages. */
    fn broadcast_task(&mut self, letter: store::Letter) -> EngineResult<bool> {
        let mut page = self.current()?.clone();
        let event = PageEvent::BroadcastMessage {
            id: letter.recipient.id,
            data: (*letter.data).clone(),
            origin: letter.origin,
        };
        let changed = self
            .broadcast_in_page(&mut page, &letter.recipient.context, &event)?
            .unwrap_or(false);
        page.scripting.broadcast.acknowledgments.push(letter.serial);
        page.scripting.broadcast_awake = true;
        page.scripting.wake();
        if changed {
            self.commit_mutation(page)?;
        } else {
            self.commit_quietly(page)?;
        }
        if self.follow_script_navigation(0).is_some() {
            return Ok(true);
        }
        Ok(changed)
    }

    /** Recursive routing restores viewport and network partition even when a child event fails. */
    fn broadcast_in_page(
        &mut self,
        page: &mut Page,
        identity: &str,
        event: &PageEvent,
    ) -> EngineResult<Option<bool>> {
        if page.script_identity == identity {
            page.scripting.wake();
            return self
                .script_step(page, serde_json::json!({"kind":"event","event":event}))
                .map(|step| Some(step.changed));
        }
        if let Some(changed) = self.deliver_worker_broadcast(page, identity, event.clone())? {
            page.scripting.wake();
            return Ok(Some(changed));
        }
        let owners: Vec<_> = page
            .frames
            .iter()
            .filter(|frame| frame.page.is_some())
            .map(|frame| frame.owner())
            .collect();
        for owner in owners {
            let dimensions = super::super::frame_pages::frame_dimensions(page, owner);
            let frame = Arc::make_mut(&mut page.frames)
                .iter_mut()
                .find(|frame| frame.owner() == owner)
                .expect("known frame");
            let child = frame.page.as_mut().expect("scripted frame");
            let viewport = self.render_context;
            let scroll = self.scroll_y;
            self.render_context.width = dimensions.0;
            self.render_context.height = dimensions.1;
            self.scroll_y = child.scripting.scroll_y.unwrap_or(0.0);
            let partition = self.frame_partition.replace(child.top_level_url.clone());
            let result = self.broadcast_in_page(child, identity, event);
            self.frame_partition = partition;
            self.render_context = viewport;
            self.scroll_y = scroll;
            if let Some(changed) = result? {
                if changed {
                    frame.follow_page(&mut page.images);
                }
                return Ok(Some(changed));
            }
        }
        Ok(None)
    }
}
