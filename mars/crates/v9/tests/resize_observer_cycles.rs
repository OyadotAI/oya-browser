/** Rendering-cycle continuations consume fresh host measurements while depth gates prevent same-frame loops. */
mod common;
use mars_core::geometry::BoxMetrics;
use mars_v9::{Execution, NodeBox, PageEnvironment, ResizeTurn, VmState};

/** A parent and child have different depths and measurable raw content/border dimensions. */
fn environment(parent: f64, child: f64) -> PageEnvironment {
    PageEnvironment {
        viewport_width: 800.0,
        viewport_height: 600.0,
        geometry: [(0, parent), (1, child)]
            .into_iter()
            .map(|(node, width)| NodeBox {
                node,
                width,
                height: 20.0,
                resize_box: Some(BoxMetrics {
                    content: [0.0, 0.0, width, 20.0],
                    border: [width, 20.0],
                    vertical: false,
                }),
                ..NodeBox::default()
            })
            .collect(),
        ..common::environment()
    }
}

/** Serialize between phases exactly as a nonresident host does; no timer API participates. */
fn phase(source: &str, env: &PageEnvironment, state: VmState, action: Option<bool>) -> Execution {
    let sources = vec![source.to_owned()];
    let host = common::host();
    let input = common::input(&sources, &host, env);
    let result = match action {
        None => mars_v9::observe_page(input, common::round_trip(state)),
        Some(complete) => {
            mars_v9::continue_resize_observers(input, common::round_trip(state), complete)
        }
    }
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    result
}

#[test]
fn deeper_targets_deliver_before_one_loop_error_and_the_next_cycle_resets_depth() {
    let source = r#"
        let sizes='', errors=0, legacy=false, eventShape=false, timer=false, intersections=0;
        setTimeout(()=>timer=true,0);
        const observedParent=document.getElementById('main'), child=document.getElementById('first');
        const observer=new ResizeObserver(entries=>{for(const e of entries)sizes+=(e.target===observedParent?'p':'c')+e.contentRect.width+'|'});
        observer.observe(observedParent);observer.observe(child);
        new IntersectionObserver(()=>intersections++).observe(child);
        addEventListener('error',e=>{errors++;eventShape=e instanceof ErrorEvent && e.isTrusted && e.cancelable && e.error===null && e.message==='ResizeObserver loop completed with undelivered notifications.'});
        onerror=(message,file,line,column,error)=>{legacy=message==='ResizeObserver loop completed with undelivered notifications.' && file==='' && line===0 && column===0 && error===null;return true};
    "#;
    let mut env = environment(100.0, 50.0);
    let initial = common::load_with(source, &env);
    let first = phase(source, &env, initial.state, None);
    assert_eq!(common::global(&first, "sizes"), "p100|c50|");
    assert_eq!(common::global(&first, "intersections"), "0");
    assert_eq!(first.effects.resize_turn, Some(ResizeTurn::Relayout));
    env = environment(120.0, 60.0);
    let deeper = phase(source, &env, first.state, Some(false));
    assert_eq!(common::global(&deeper, "sizes"), "p100|c50|c60|");
    let error = phase(source, &env, deeper.state, Some(false));
    assert_eq!(common::global(&error, "errors"), "1");
    assert_eq!(common::global(&error, "legacy"), "true");
    assert_eq!(common::global(&error, "eventShape"), "true");
    assert_eq!(common::global(&error, "intersections"), "0");
    assert!(
        !error
            .effects
            .console
            .iter()
            .any(|entry| entry.text.contains("undelivered notifications"))
    );
    let completed = phase(source, &env, error.state, Some(false));
    assert_eq!(
        completed.effects.resize_turn,
        Some(ResizeTurn::Complete { deferred: true })
    );
    assert_eq!(common::global(&completed, "intersections"), "1");
    assert_eq!(common::global(&completed, "timer"), "false");
    let next = phase(source, &env, completed.state, None);
    assert_eq!(common::global(&next, "sizes"), "p100|c50|c60|p120|");
    let final_turn = phase(source, &env, next.state, Some(false));
    assert_eq!(
        final_turn.effects.resize_turn,
        Some(ResizeTurn::Complete { deferred: false })
    );
    assert_eq!(common::global(&final_turn, "errors"), "1");
    assert_eq!(common::global(&final_turn, "timer"), "false");
}

#[test]
fn deadline_finalization_preserves_undelivered_targets_without_another_broadcast() {
    let source = "let calls=0,errors=0;new ResizeObserver(()=>calls++).observe(document.getElementById('first'));addEventListener('error',()=>errors++);";
    let mut env = environment(100.0, 50.0);
    let initial = common::load_with(source, &env);
    let first = phase(source, &env, initial.state, None);
    env = environment(100.0, 70.0);
    let stopped = phase(source, &env, first.state, Some(true));
    assert_eq!(common::global(&stopped, "calls"), "1");
    assert_eq!(common::global(&stopped, "errors"), "1");
    let completed = phase(source, &env, stopped.state, Some(true));
    assert_eq!(
        completed.effects.resize_turn,
        Some(ResizeTurn::Complete { deferred: true })
    );
    let next = phase(source, &env, completed.state, None);
    assert_eq!(common::global(&next, "calls"), "2");
}

#[test]
fn increasing_depth_cannot_bypass_the_cross_process_broadcast_cap() {
    let count = mars_v9::RESIZE_BROADCAST_LIMIT + 1;
    let source = format!(
        "let calls=0,errors=0;const observer=new ResizeObserver(()=>{{calls++;if(calls<{count})observer.observe(document.getElementById('n'+calls))}});observer.observe(document.getElementById('n0'));addEventListener('error',()=>errors++);"
    );
    let mut document = common::page();
    document.nodes = (0..count)
        .map(|node| {
            common::element(
                node,
                node.checked_sub(1),
                "div",
                &[("id", &format!("n{node}"))],
            )
        })
        .collect();
    let mut env = environment(100.0, 50.0);
    let template = env.geometry[0].clone();
    env.geometry = (0..count)
        .map(|node| NodeBox {
            node,
            ..template.clone()
        })
        .collect();
    let sources = vec![source];
    let host = common::host();
    let input = || {
        let mut input = common::input(&sources, &host, &env);
        input.document = document.clone();
        input
    };
    let mut result = mars_v9::load_page(input()).unwrap();
    for round in 0..=mars_v9::RESIZE_BROADCAST_LIMIT + 1 {
        result = if round == 0 {
            mars_v9::observe_page(input(), common::round_trip(result.state))
        } else {
            mars_v9::continue_resize_observers(input(), common::round_trip(result.state), false)
        }
        .unwrap();
        if matches!(
            result.effects.resize_turn,
            Some(ResizeTurn::Complete { .. })
        ) {
            break;
        }
    }
    assert_eq!(
        common::global(&result, "calls"),
        mars_v9::RESIZE_BROADCAST_LIMIT.to_string()
    );
    assert_eq!(common::global(&result, "errors"), "1");
    assert_eq!(
        result.effects.resize_turn,
        Some(ResizeTurn::Complete { deferred: true })
    );
}
