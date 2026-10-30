/** RoundRect radii conversion obeys sequence, dictionary, coercion and range rules before constructing real pixels. */
mod common;
use mars_core::canvas::CanvasHost;
use mars_v9::DocumentHost;

/** Pure path rendering does not require a native graphics or network service. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Run ordinary JavaScript against the public Canvas context and inspect its readback. */
fn run(script: &str) -> String {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!(
        "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');const alpha=(x,y)=>a.getImageData(x,y,1,1).data[3];{script}"
    )];
    let result = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&result), None);
    common::global(&result, "r")
}

/** Numeric, point-dictionary and iterable inputs all produce genuine rounded corners. */
#[test]
fn round_rect_accepts_numbers_points_and_sequences() {
    for radii in ["3", "{x:3,y:3}", "[3]", "[{x:3,y:3},3]", "new Set([3])"] {
        assert_eq!(
            run(&format!(
                "a.roundRect(0,0,8,8,{radii});a.fill();const r=[alpha(0,0),alpha(3,3)].join('|')"
            )),
            "0|255"
        );
    }
}

/** Undefined and null use zero radii while dictionary members default independently. */
#[test]
fn omitted_and_defaulted_radii_produce_square_rectangles() {
    for radius in ["undefined", "null", "{}", "{x:0,y:3}", "{x:3}"] {
        assert_eq!(
            run(&format!(
                "a.roundRect(0,0,8,8,{radius});a.fill();const r=[alpha(0,0),alpha(7,7)].join('|')"
            )),
            "255|255"
        );
    }
}

/** DOMPointInit converts all members in lexical order, including w and z which do not affect corner geometry. */
#[test]
fn point_dictionary_members_are_converted_in_order() {
    assert_eq!(
        run(
            "let order='';const p={get w(){order+='w';return 1},get x(){order+='x';return 3},get y(){order+='y';return 3},get z(){order+='z';return 0}};a.roundRect(0,0,8,8,p);a.fill();const r=order+'|'+alpha(0,0)+'|'+alpha(3,3)"
        ),
        "wxyz|0|255"
    );
}

/** Each yielded dictionary is converted before the next iterator step is observable. */
#[test]
fn radii_sequence_conversion_interleaves_with_iteration() {
    assert_eq!(
        run(
            "let order='',i=0;const p={get x(){order+='x';return 3},get y(){order+='y';return 3}};const radii={[Symbol.iterator](){return {next(){order+='n';return i++===0?{value:p,done:false}:{done:true}}}}};a.roundRect(0,0,8,8,radii);const r=order"
        ),
        "nxyn"
    );
}

/** Item-conversion exceptions close an iterator while retaining the original thrown value. */
#[test]
fn failed_radii_conversion_closes_the_iterator() {
    assert_eq!(
        run(
            "let closed=0,r='';const radii={[Symbol.iterator](){return {next(){return {done:false,value:{get x(){throw 42}}}},return(){closed++;throw 99}}}};try{a.roundRect(0,0,8,8,radii)}catch(e){r=e+'|'+closed}"
        ),
        "42|1"
    );
}

/** Rectangle finiteness precedes algorithmic radius-count errors, after all WebIDL conversion. */
#[test]
fn radius_count_and_sign_errors_follow_geometry_and_input_order() {
    assert_eq!(
        run(
            "let r='';a.roundRect(NaN,0,8,8,[]);for(const radii of [[],[1,2,3,4,5],-1,[1,-1]]){try{a.roundRect(0,0,8,8,radii)}catch(e){r+=e.name+'|'}}a.roundRect(0,0,8,8,[NaN,-1]);try{a.roundRect(0,0,8,8,[-1,NaN])}catch(e){r+=e.name}"
        ),
        "RangeError|RangeError|RangeError|RangeError|RangeError"
    );
}

/** More than four items are still converted before RangeError rather than suppressing later getters. */
#[test]
fn overlong_finite_sequence_finishes_conversion_before_range_error() {
    assert_eq!(
        run(
            "let r='',seen=0;try{a.roundRect(0,0,8,8,[1,2,3,4,{get x(){seen++;return 5}}])}catch(e){r=e.name+'|'+seen}"
        ),
        "RangeError|1"
    );
}

/** A radii getter can reset the context, so the resulting geometry uses the final transform. */
#[test]
fn radii_conversion_reacquires_the_current_context() {
    assert_eq!(
        run(
            "a.translate(6,0);a.roundRect(0,0,8,8,{get x(){c.width=8;return 3},y:3});a.fill();const r=[alpha(3,3),alpha(0,0)].join('|')"
        ),
        "255|0"
    );
}

/** Infinite iterables are bounded and closed, and a later valid draw can still succeed. */
#[test]
fn radii_iterable_work_is_bounded_and_failure_leaves_the_path_usable() {
    assert_eq!(
        run(
            "let r='',closed=0;const radii={[Symbol.iterator](){return {next(){return {value:1,done:false}},return(){closed++;return {done:true}}}}};try{a.roundRect(0,0,8,8,radii)}catch(e){r=e.name+'|'+closed}a.roundRect(0,0,1,1);a.fill();r+='|'+alpha(0,0)"
        ),
        "QuotaExceededError|1|255"
    );
}
