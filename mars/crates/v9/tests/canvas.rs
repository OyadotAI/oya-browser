/** Canvas public APIs use real bounded pixels and retain identity independently of native text. */
mod common;
use mars_core::canvas::CanvasHost;
use mars_v9::{DocumentHost, PageInput, StorageState};

/** Pixel-only test host isolates VM semantics; native glyph/PNG integration is tested by Mars. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}
/** Execute using a host that admits Canvas, while checking uncaught failures explicitly. */
fn run(source: &str) -> String {
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let execution = mars_v9::load_page(PageInput {
        sources: &[source.into()],
        source_keys: &[],
        document: common::page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .unwrap();
    assert_eq!(common::failure(&execution), None);
    common::global(&execution, "r")
}
#[test]
fn context_is_canvas_owned_and_unsupported_contexts_are_null() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas');const a=c.getContext('2d');const r=[a===c.getContext('2d'),a.canvas===c,c.getContext('webgl')===null,typeof document.createElement('div').getContext].join('|');"
        ),
        "true|true|true|undefined"
    );
}
#[test]
fn actual_pixels_are_read_back_as_a_separate_clamped_array() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas');c.width=4;c.height=4;const a=c.getContext('2d');a.fillStyle='#ff0000';a.fillRect(0,0,4,4);a.clearRect(1,1,1,1);const d=a.getImageData(0,0,2,2);const r=[d.data instanceof Uint8ClampedArray,Array.from(d.data).join(',')].join('|');"
        ),
        "true|255,0,0,255,255,0,0,255,255,0,0,255,0,0,0,0"
    );
}
#[test]
fn same_value_resize_resets_pixels_and_state_but_preserves_context() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas');const a=c.getContext('2d');a.fillStyle='red';a.fillRect(0,0,1,1);c.width=c.width;const r=[a===c.getContext('2d'),a.fillStyle,Array.from(a.getImageData(0,0,1,1).data)].join('|');"
        ),
        "true|#000000|0,0,0,0"
    );
}
#[test]
fn excessive_size_releases_backing_and_valid_resize_recovers() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas');const a=c.getContext('2d');c.width=513;let error='';try{a.fillRect(0,0,1,1)}catch(e){error=e.name}const empty=c.toDataURL();c.width=2;c.height=2;a.fillStyle='blue';a.fillRect(0,0,1,1);const r=[error,empty,a===c.getContext('2d'),Array.from(a.getImageData(0,0,1,1).data)].join('|');"
        ),
        "InvalidStateError|data:,|true|0,0,255,255"
    );
}
#[test]
fn readback_pads_outside_pixels_and_normalizes_negative_extents() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas');c.width=1;c.height=1;const a=c.getContext('2d');a.fillStyle='white';a.fillRect(0,0,1,1);const r=Array.from(a.getImageData(1,0,-2,1).data).join(',');"
        ),
        "0,0,0,0,255,255,255,255"
    );
}
#[test]
fn separate_canvases_do_not_share_pixels() {
    assert_eq!(
        run(
            "const c=document.createElement('canvas'),d=document.createElement('canvas');const a=c.getContext('2d'),b=d.getContext('2d');a.fillRect(0,0,1,1);const r=[a===b,a.getImageData(0,0,1,1).data[3],b.getImageData(0,0,1,1).data[3]].join('|');"
        ),
        "false|255|0"
    );
}
#[test]
fn missing_operations_are_explicit_and_gpu_discovery_is_a_promise() {
    assert_eq!(
        run(
            "const a=document.createElement('canvas').getContext('2d');let error='';try{a.drawImage({})}catch(e){error=e.name}let r=error;const p=navigator.gpu.requestAdapter();p.then(a=>{r+='|'+(p instanceof Promise)+'|'+String(a)});"
        ),
        "NotSupportedError|true|null"
    );
}
#[test]
fn standalone_host_does_not_claim_native_graphics() {
    assert_eq!(
        common::eval(
            "const r=document.createElement('canvas').getContext('2d')===null;",
            "r"
        ),
        "true"
    );
}

/** Serialized requests and GC preserve the real context/owner cycle only while reachable. */
#[test]
fn context_and_pixels_survive_a_serialized_request() {
    let sources = vec!["const c=document.createElement('canvas'),a=c.getContext('2d');a.fillStyle='red';a.fillRect(0,0,1,1);let r='';window.onload=()=>{r=[a===c.getContext('2d'),a.canvas===c,Array.from(a.getImageData(0,0,1,1).data)].join('|')};".into()];
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let saved = common::round_trip(loaded.state);
    let resumed = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        saved,
        mars_v9::PageEvent::Load,
    )
    .unwrap();
    assert_eq!(common::failure(&resumed), None);
    assert_eq!(common::global(&resumed, "r"), "true|true|255,0,0,255");
}

/** Unreachable detached canvas/context cycles must not exhaust later requests. */
#[test]
fn detached_contexts_are_reclaimed_between_requests() {
    let sources = vec!["for(let i=0;i<16;i++){const c=document.createElement('canvas');c.foo=1;c.getContext('2d')}let r='';window.onload=()=>{for(let i=0;i<16;i++)document.createElement('canvas').getContext('2d');r='reclaimed'};".into()];
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&loaded), None);
    let resumed = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        loaded.state,
        mars_v9::PageEvent::Load,
    )
    .unwrap();
    assert_eq!(common::failure(&resumed), None);
    assert_eq!(common::global(&resumed, "r"), "reclaimed");
}

/** Dropped listeners cannot keep captured canvas resources alive across requests. */
#[test]
fn unreachable_listener_cycles_release_canvas_quota() {
    for setup in [
        "c.addEventListener('click',()=>c.width)",
        "const target=new EventTarget();target.addEventListener('click',()=>c.width)",
    ] {
        let sources = vec![format!(
            "function make(){{const c=document.createElement('canvas');c.getContext('2d');{setup}}}for(let i=0;i<16;i++)make();let r='';window.onload=()=>{{for(let i=0;i<16;i++)make();r='recovered'}};"
        )];
        let host = DocumentHost {
            imports: None,
            canvas: &Pixels,
            ..common::host()
        };
        let environment = common::environment();
        let mut execution =
            mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
        assert_eq!(common::failure(&execution), None);
        for _ in 0..3 {
            execution = mars_v9::page_event(
                common::input(&sources, &host, &environment),
                common::round_trip(execution.state),
                mars_v9::PageEvent::Load,
            )
            .unwrap();
            assert_eq!(common::failure(&execution), None);
            assert_eq!(common::global(&execution, "r"), "recovered");
        }
    }
}

/** Retained detached targets preserve callbacks, abort signals and drawing state. */
#[test]
fn reachable_listener_keeps_canvas_pixels_and_abort_signal() {
    let sources = vec!["const parent=document.createElement('div'),controller=new AbortController();{const c=document.createElement('canvas');parent.appendChild(c);c.getContext('2d').fillRect(0,0,1,1);c.addEventListener('click',()=>{r+=c.getContext('2d').getImageData(0,0,1,1).data[3]}, {signal:controller.signal})}let r='';window.onload=()=>{parent.firstChild.dispatchEvent(new Event('click'));controller.abort();parent.firstChild.dispatchEvent(new Event('click'))};".into()];
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let mut environment = common::environment();
    environment.dom_level = mars_v9::DomLevel::Extended;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&loaded), None);
    let resumed = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        mars_v9::PageEvent::Load,
    )
    .unwrap();
    assert_eq!(common::failure(&resumed), None);
    assert_eq!(common::global(&resumed, "r"), "255");
}

/** A live detached parent retains its canvas child, context, pixels and expando across requests. */
#[test]
fn reachable_detached_tree_keeps_canvas_state() {
    let sources = vec!["const parent=document.createElement('div');{const c=document.createElement('canvas');c.foo=7;parent.appendChild(c);c.getContext('2d').fillRect(0,0,1,1)}let r='';window.onload=()=>{const c=parent.firstChild;r=c.foo+'|'+c.getContext('2d').getImageData(0,0,1,1).data[3]};".into()];
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let mut environment = common::environment();
    environment.dom_level = mars_v9::DomLevel::Extended;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&loaded), None);
    let resumed = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        mars_v9::PageEvent::Load,
    )
    .unwrap();
    assert_eq!(common::failure(&resumed), None);
    assert_eq!(common::global(&resumed, "r"), "7|255");
}

/** WeakMap values can reveal DOM edges, which can in turn keep further weak keys alive. */
#[test]
fn weak_map_reachable_tree_keeps_canvas_state() {
    let sources = vec!["const key={},wm=new WeakMap();function make(){const p=document.createElement('div'),c=document.createElement('canvas');c.foo=7;p.appendChild(c);c.getContext('2d').fillRect(0,0,1,1);return p}wm.set(key,make());let r='';window.onload=()=>{const c=wm.get(key).firstChild;r=c.foo+'|'+c.getContext('2d').getImageData(0,0,1,1).data[3]};".into()];
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let mut environment = common::environment();
    environment.dom_level = mars_v9::DomLevel::Extended;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&loaded), None);
    let resumed = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        mars_v9::PageEvent::Load,
    )
    .unwrap();
    assert_eq!(common::failure(&resumed), None);
    assert_eq!(common::global(&resumed, "r"), "7|255");
}

#[test]
fn aggregate_allocation_failure_is_catchable_and_preserves_pixels() {
    assert_eq!(
        run(
            "const held=[];for(let i=0;i<4;i++){const c=document.createElement('canvas');c.width=512;c.height=512;held.push(c.getContext('2d'))}held[0].fillRect(0,0,1,1);let error='';try{document.createElement('canvas').getContext('2d')}catch(e){error=e.name}const r=error+'|'+held[0].getImageData(0,0,1,1).data[3];"
        ),
        "QuotaExceededError|255"
    );
}

#[test]
fn borrowed_graphics_methods_reject_unbranded_receivers() {
    assert_eq!(
        run(
            "let r='';try{HTMLCanvasElement.prototype.getContext.call({},'2d')}catch(e){r+=e.name}try{GPU.prototype.requestAdapter.call({})}catch(e){r+='|'+e.name}"
        ),
        "TypeError|TypeError"
    );
}

/** Canvas adoption during coercion must re-resolve the receiver instead of indexing a stale node. */
#[test]
fn adoption_during_argument_conversion_keeps_the_context_and_pixels() {
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let mut environment = common::environment();
    environment.dom_level = mars_v9::DomLevel::Extended;
    let sources=vec!["const c=document.createElement('canvas'),a=c.getContext('2d');const x={valueOf(){document.getElementById('first').appendChild(c);return 0}};a.fillRect(x,0,1,1);const r=[a===c.getContext('2d'),a.canvas===c,a.getImageData(0,0,1,1).data[3],a.getImageData(0,0,1,1) instanceof ImageData].join('|');".into()];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&loaded), None);
    assert_eq!(common::global(&loaded, "r"), "true|true|255|true");
    assert!(
        loaded
            .script_changes
            .iter()
            .any(|change| matches!(change, mars_core::dom::ScriptChange::Canvas { .. }))
    );
}
