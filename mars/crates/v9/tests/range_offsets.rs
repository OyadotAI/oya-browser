/** Range offsets are WebIDL unsigned longs: a negative offset wraps to a large value and so throws IndexSizeError rather than being clamped to zero. */
mod common;
use common::eval;

/** comparePoint with offset -1 wraps to 4294967295, which is past the node's length, so it throws. */
#[test]
fn compare_point_negative_offset_throws_index_size() {
    let source = "const p=document.createElement('p');p.append('ab');\
        const range=document.createRange();range.selectNodeContents(p);\
        let r='none';try{range.comparePoint(p.firstChild,-1)}catch(e){r=e.name}";
    assert_eq!(eval(source, "r"), "IndexSizeError");
}
