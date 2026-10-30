/** Native geometry invalidation schedules observer-only turns without polling or waking author timers. */
use super::page::Page;
use crate::document::RenderContext;
use crate::v9::{NodeBox, PageEnvironment};

/** One page candidate retains only its last sampled geometry and whether delivery is needed. */
#[derive(Clone, Default)]
pub(super) struct Observations {
    /// Registrations or queued records still exist in this page's V9 realm.
    active: bool,
    /// Native geometry changed or the realm retained asynchronous delivery records.
    pub pending: bool,
    /// Comparing only at layout boundaries avoids rebuilding boxes on every idle tick.
    sampled: Option<Geometry>,
}

/** Intersection input is geometry, viewport and scroll, independent of paint-only opacity changes. */
#[derive(Clone, PartialEq)]
struct Geometry {
    boxes: Vec<NodeBox>,
    viewport: [f64; 2],
    scroll: [f64; 2],
}

impl Observations {
    /** Record what the accepted V9 candidate actually sampled, not its later mutated layout. */
    pub fn sampled(&mut self, environment: &PageEnvironment, active: bool, queued: bool) {
        self.active = active;
        self.pending = active && queued;
        self.sampled = active.then(|| Geometry {
            boxes: environment.geometry.clone(),
            viewport: [environment.viewport_width, environment.viewport_height],
            scroll: [environment.scroll_x, environment.scroll_y],
        });
    }

    /** Scrolling already has an external trigger and needs no perpetual background timer. */
    pub fn invalidate(&mut self) {
        self.pending |= self.active;
    }

    /** Unchanged boxes never keep an opacity animation's observer awake. */
    fn layout(&mut self, geometry: Geometry) {
        self.pending |= self.active && self.sampled.as_ref() != Some(&geometry);
    }
}

/** Both early and committed layout paths compare their own candidate's exact viewport and boxes. */
pub(super) fn layout(page: &mut Page, context: RenderContext, scroll_y: f64) {
    if !page.scripting.intersections.active {
        return;
    }
    let geometry = Geometry {
        boxes: page
            .display
            .as_ref()
            .map(super::scripting::boxes)
            .unwrap_or_default(),
        viewport: [context.width, context.height],
        scroll: [
            page.scripting.scroll_x.unwrap_or(page.scroll_x),
            page.scripting.scroll_y.unwrap_or(scroll_y),
        ],
    };
    page.scripting.intersections.layout(geometry);
}

#[cfg(test)]
mod tests {
    use super::*;

    /** An idle observer needs one turn per changed layout, and no turn for unchanged presentation. */
    #[test]
    fn geometry_changes_schedule_once_and_unchanged_layout_stays_idle() {
        let environment = PageEnvironment::default();
        let mut observations = Observations::default();
        observations.sampled(&environment, true, false);
        let mut geometry = observations.sampled.clone().unwrap();
        observations.layout(geometry.clone());
        assert!(!observations.pending);
        geometry.viewport[0] += 1.0;
        observations.layout(geometry);
        assert!(observations.pending);
        observations.sampled(&environment, true, false);
        assert!(!observations.pending);
    }

    /** Queued records survive unchanged geometry; disposing the last registration and queue stops work. */
    #[test]
    fn queued_delivery_remains_pending_until_consumed() {
        let environment = PageEnvironment::default();
        let mut observations = Observations::default();
        observations.sampled(&environment, true, true);
        observations.layout(observations.sampled.clone().unwrap());
        assert!(observations.pending);
        observations.sampled(&environment, false, false);
        observations.invalidate();
        assert!(!observations.pending);
        assert!(observations.sampled.is_none());
    }
}
