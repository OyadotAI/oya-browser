/** Layout sampling coalesces current sizes and never invents a content box from an unrelated border rectangle. */
use super::*;

/** Sample once per host turn; author callbacks that alter layout are remeasured by the next native layout. */
pub(in crate::v9) fn sample(vm: &mut Vm) -> Res<()> {
    let ids: Vec<_> = vm.page.state.resize_observers.keys().copied().collect();
    for id in ids {
        let mut state = vm.page.state.resize_observers[&id].clone();
        state.queued.clear();
        for target in &state.targets {
            if let Some(entry) = measure(vm, target)?
                && entry.observed != target.previous
            {
                state.queued.push(entry);
            }
        }
        vm.page.state.resize_observers.insert(id, state);
    }
    vm.page.effects.resize_observers = vm
        .page
        .state
        .resize_observers
        .values()
        .any(|state| !state.targets.is_empty());
    vm.page.effects.resize_observers_pending = vm
        .page
        .state
        .resize_observers
        .values()
        .any(|state| !state.queued.is_empty());
    Ok(())
}

/** Disconnected and laid-out absent boxes are genuinely zero; unmeasured connected boxes remain pending. */
fn measure(vm: &mut Vm, target: &Observation) -> Res<Option<Entry>> {
    let connected = vm.page.connected(target.node);
    let environment = &vm.page.environment;
    let measured = environment
        .geometry
        .iter()
        .find(|bounds| bounds.node == target.node && bounds.resize_box.is_some())
        .and_then(|bounds| bounds.resize_box);
    let no_box = !connected
        || !environment
            .geometry
            .iter()
            .any(|bounds| bounds.node == target.node)
            && environment
                .laid_out_before
                .is_some_and(|before| target.node < before);
    let metrics = if no_box {
        mars_core::geometry::BoxMetrics {
            content: [0.0; 4],
            border: [0.0; 2],
            vertical: false,
        }
    } else if let Some(measured) = measured {
        measured
    } else {
        return Ok(None);
    };
    if !metrics.valid()
        || !environment.device_pixel_ratio.is_finite()
        || environment.device_pixel_ratio <= 0.0
    {
        return Err(EngineError::invalid("Invalid ResizeObserver measurements.").into());
    }
    let logical = |[width, height]| {
        if metrics.vertical {
            [height, width]
        } else {
            [width, height]
        }
    };
    let content_size = logical([metrics.content[2], metrics.content[3]]);
    let border_size = logical(metrics.border);
    let device_size = content_size.map(|value| (value * environment.device_pixel_ratio).round());
    if device_size
        .iter()
        .any(|value| !value.is_finite() || *value > SIZE_LIMIT)
    {
        return Err(EngineError::invalid("Invalid ResizeObserver device size.").into());
    }
    let observed = match target.kind {
        BoxKind::Content => content_size,
        BoxKind::Border => border_size,
        BoxKind::DevicePixelContent => device_size,
    };
    Ok(Some(Entry {
        node: target.node,
        target: target.object,
        content: metrics.content,
        content_size,
        border_size,
        device_size,
        observed,
    }))
}
