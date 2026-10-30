/** Native geometry invalidation schedules observer-only turns without polling or waking author timers. */
use super::page::Page;
use crate::document::RenderContext;
use crate::v9::{NodeBox, PageEnvironment};

/** One page candidate retains only its last sampled geometry and whether delivery is needed. */
#[derive(Clone, Default)]
pub(super) struct Observations {
    /// Registrations or queued records still exist in this page's V9 realm.
    active: bool,
    /// Resize observers compare raw measured boxes, independently of visual geometry.
    resize_active: bool,
    /// Native geometry changed or the realm retained asynchronous delivery records.
    pub pending: bool,
    /// Comparing only at layout boundaries avoids rebuilding boxes on every idle tick.
    sampled: Option<Geometry>,
}

/** Intersection input is geometry, viewport and scroll, independent of paint-only opacity changes. */
#[derive(Clone, PartialEq)]
struct Geometry {
    boxes: Vec<NodeBox>,
    wrappers: mars_core::geometry::WrapperMetrics,
    viewport: [f64; 2],
    scroll: [f64; 2],
    device_pixel_ratio: f64,
}

impl Observations {
    /** Record what the accepted V9 candidate actually sampled, not its later mutated layout. */
    pub fn sampled(
        &mut self,
        environment: &PageEnvironment,
        active: bool,
        queued: bool,
        resize_active: bool,
        resize_queued: bool,
    ) {
        self.active = active;
        self.resize_active = resize_active;
        self.pending = active && queued || resize_active && resize_queued;
        self.sampled = (active || resize_active).then(|| Geometry {
            boxes: environment.geometry.clone(),
            wrappers: environment.resize_wrappers,
            viewport: [environment.viewport_width, environment.viewport_height],
            scroll: [environment.scroll_x, environment.scroll_y],
            device_pixel_ratio: environment.device_pixel_ratio,
        });
    }

    /** Scrolling already has an external trigger and needs no perpetual background timer. */
    pub fn invalidate(&mut self) {
        self.pending |= self.active;
    }

    /** A resized native child needs layout before either observer family can compare fresh measurements. */
    pub fn invalidate_layout(&mut self) {
        self.pending |= self.active || self.resize_active;
    }

    /** Unchanged boxes never keep an opacity animation's observer awake. */
    fn layout(&mut self, geometry: Geometry) {
        self.pending |= self.active && self.sampled.as_ref() != Some(&geometry);
        self.pending |= self.resize_active
            && self.sampled.as_ref().is_none_or(|previous| {
                previous.device_pixel_ratio != geometry.device_pixel_ratio
                    || previous.wrappers != geometry.wrappers
                    || resize_boxes(previous) != resize_boxes(&geometry)
            });
    }
}

/** Both early and committed layout paths compare their own candidate's exact viewport and boxes. */
pub(super) fn layout(page: &mut Page, context: RenderContext, scroll_y: f64) {
    if !page.scripting.intersections.active && !page.scripting.intersections.resize_active {
        return;
    }
    let geometry = Geometry {
        boxes: page
            .display
            .as_ref()
            .map(super::scripting::boxes)
            .unwrap_or_default(),
        wrappers: page.display.as_ref().map(|display| display.resize_wrappers).unwrap_or_default(),
        viewport: [context.width, context.height],
        device_pixel_ratio: page.scripting.environment.device_pixel_ratio,
        scroll: [
            page.scripting.scroll_x.unwrap_or(page.scroll_x),
            page.scripting.scroll_y.unwrap_or(scroll_y),
        ],
    };
    page.scripting.intersections.layout(geometry);
}

/** Text echoes and generated fragments cannot restart observations of the same native layout box. */
fn resize_boxes(
    geometry: &Geometry,
) -> std::collections::BTreeMap<usize, mars_core::geometry::BoxMetrics> {
    let mut boxes = std::collections::BTreeMap::new();
    for bounds in &geometry.boxes {
        if let Some(measured) = bounds.resize_box {
            boxes.entry(bounds.node).or_insert(measured);
        }
    }
    boxes
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Pure paint movement and viewport scrolling cannot wake observers of unchanged raw sizes. */
    #[test]
    fn resize_only_observers_wait_for_raw_measurement_changes() {
        let mut environment = PageEnvironment::default();
        environment.geometry.push(NodeBox {
            node: 1,
            resize_box: Some(mars_core::geometry::BoxMetrics {
                content: [0.0, 0.0, 100.0, 50.0],
                border: [100.0, 50.0],
                vertical: false,
            }),
            ..NodeBox::default()
        });
        let mut observations = Observations::default();
        observations.sampled(&environment, false, false, true, false);
        let mut geometry = observations.sampled.clone().unwrap();
        geometry.boxes[0].x = 500.0;
        geometry.boxes[0].width = 500.0;
        geometry.scroll[1] = 100.0;
        observations.layout(geometry.clone());
        observations.invalidate();
        assert!(!observations.pending);
        geometry.boxes[0].resize_box.as_mut().unwrap().content[2] = 120.0;
        observations.layout(geometry);
        assert!(observations.pending);
    }

    /** An idle observer needs one turn per changed layout, and no turn for unchanged presentation. */
    #[test]
    fn geometry_changes_schedule_once_and_unchanged_layout_stays_idle() {
        let environment = PageEnvironment::default();
        let mut observations = Observations::default();
        observations.sampled(&environment, true, false, false, false);
        let mut geometry = observations.sampled.clone().unwrap();
        observations.layout(geometry.clone());
        assert!(!observations.pending);
        geometry.viewport[0] += 1.0;
        observations.layout(geometry);
        assert!(observations.pending);
        observations.sampled(&environment, true, false, false, false);
        assert!(!observations.pending);
    }

    /** Queued records survive unchanged geometry; disposing the last registration and queue stops work. */
    #[test]
    fn queued_delivery_remains_pending_until_consumed() {
        let environment = PageEnvironment::default();
        let mut observations = Observations::default();
        observations.sampled(&environment, true, true, false, false);
        observations.layout(observations.sampled.clone().unwrap());
        assert!(observations.pending);
        observations.sampled(&environment, false, false, false, false);
        observations.invalidate();
        assert!(!observations.pending);
        assert!(observations.sampled.is_none());
    }
}
