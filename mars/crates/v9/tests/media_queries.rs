/** Script `matchMedia` answers width and height queries the way stylesheets do. */
use crate::common;

/** Without a host evaluator, V9's own subset still knows heights: Airbnb decides its whole
layout from `(min-width: 744px) and (min-height: 480px)`, which used to answer false. */
#[test]
fn match_media_answers_width_and_height_queries() {
    let source = "const q=s=>matchMedia(s).matches;\
        const r=[q('(min-width: 744px) and (min-height: 480px)'),q('(min-height: 480px)'),\
        q('(max-height: 100px)'),q('(min-width: 99999px)'),q('screen and (min-width: 300px)')].join();";
    assert_eq!(common::eval(source, "r"), "true,true,false,false,true");
}
