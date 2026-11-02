/** Live ranges move with the tree, and range contents follow the DOM standard's extract and clone rules. */
use crate::common;
use common::eval;

/** A removed node's ancestor boundary collapses to where the node sat. */
#[test]
fn live_range_point_inside_a_removed_node_collapses_to_its_place() {
    let source = "const d=document.createElement('div');const a=document.createElement('a');d.append(a);\
        const t=document.createTextNode('abc');a.appendChild(t);const r=document.createRange();r.setStart(t,1);r.setEnd(t,2);\
        d.removeChild(a);const out=[r.startContainer===d,r.startOffset,r.endContainer===d,r.endOffset].join();";
    assert_eq!(eval(source, "out"), "true,0,true,0");
}

/** A boundary after an inserted node moves past it, and one before it stays put. */
#[test]
fn live_range_offset_after_an_insertion_point_moves_on() {
    let source = "const d=document.createElement('div');const a=document.createElement('a');d.append(a);\
        const r=document.createRange();r.setStart(d,1);r.setEnd(d,1);\
        d.insertBefore(document.createElement('i'),a);const out=[r.startOffset,r.endOffset].join();";
    assert_eq!(eval(source, "out"), "2,2");
}

/** A boundary inside replaced character data snaps to the span start; one past it shifts. */
#[test]
fn live_range_points_in_deleted_text_snap_to_the_deletion() {
    let source = "const t=document.createTextNode('abcdef');const r=document.createRange();r.setStart(t,2);r.setEnd(t,5);\
        t.deleteData(1,3);const out=[r.startOffset,r.endOffset,t.data].join();";
    assert_eq!(eval(source, "out"), "1,2,aef");
}

/** splitText moves the points past the split into the new node and steps the parent's point past it. */
#[test]
fn split_text_moves_points_into_the_new_node() {
    let source = "const d=document.createElement('div');const t=document.createTextNode('abcd');d.append(t);\
        const r=document.createRange();r.setStart(t,3);r.setEnd(d,1);const n=t.splitText(1);\
        const out=[r.startContainer===n,r.startOffset,r.endContainer===d,r.endOffset,t.data,n.data].join();";
    assert_eq!(eval(source, "out"), "true,2,true,2,a,bcd");
}

/** normalize moves the points of a merged text node into the text it joins. */
#[test]
fn normalize_moves_points_of_a_merged_text_node() {
    let source = "const d=document.createElement('div');const t1=document.createTextNode('ab');const t2=document.createTextNode('cd');\
        d.append(t1,t2);const r=document.createRange();r.setStart(t2,1);r.setEnd(d,2);d.normalize();\
        const out=[r.startContainer===t1,r.startOffset,r.endContainer===d,r.endOffset,d.childNodes.length,t1.data].join();";
    assert_eq!(eval(source, "out"), "true,3,true,1,1,abcd");
}

/** extractContents copies a partly selected element shallowly and moves its selected part into the copy. */
#[test]
fn extract_contents_keeps_the_markup_around_a_partial_element() {
    let source = "const d=document.createElement('div');d.innerHTML='<p>ab<b>cd</b></p><i>x</i>';\
        const r=document.createRange();r.setStart(d.firstChild.firstChild,1);r.setEnd(d,1);\
        const f=r.extractContents();\
        const out=[f.childNodes.length,f.firstChild.outerHTML,d.innerHTML,r.startContainer===d,r.startOffset].join('|');";
    assert_eq!(
        eval(source, "out"),
        "1|<p>b<b>cd</b></p>|<p>a</p><i>x</i>|true|1"
    );
}

/** cloneContents copies the contained nodes and leaves both the tree and the range as they were. */
#[test]
fn clone_contents_leaves_the_tree_and_range_unchanged() {
    let source = "const d=document.createElement('div');d.innerHTML='<p>ab</p><p>cd</p>';\
        const r=document.createRange();r.setStart(d,0);r.setEnd(d,2);const f=r.cloneContents();\
        const out=[f.childNodes.length,d.childNodes.length,r.startOffset,r.endOffset].join();";
    assert_eq!(eval(source, "out"), "2,2,0,2");
}

/** compareBoundaryPoints between ranges in different trees throws WrongDocumentError. */
#[test]
fn compare_boundary_points_across_trees_throws_wrong_document() {
    let source = "const d=document.createElement('div');const r1=document.createRange();const r2=document.createRange();\
        r2.setStart(d,0);r2.setEnd(d,0);let name='none';\
        try{r1.compareBoundaryPoints(0,r2)}catch(e){name=e.name}const out=name;";
    assert_eq!(eval(source, "out"), "WrongDocumentError");
}

/** A StaticRange keeps its points when the tree changes underneath it. */
#[test]
fn static_range_does_not_follow_mutations() {
    let source = "const d=document.createElement('div');const a=document.createElement('a');\
        d.append(a,document.createElement('b'));\
        const s=new StaticRange({startContainer:d,startOffset:1,endContainer:d,endOffset:2});\
        d.removeChild(a);const out=[s.startOffset,s.endOffset,s.collapsed,s instanceof StaticRange].join();";
    assert_eq!(eval(source, "out"), "1,2,false,true");
}

/** createCDATASection is refused by the page's HTML document and allowed by another document. */
#[test]
fn create_cdata_section_is_refused_by_html_documents_only() {
    let source = "let name='ok';try{document.createCDATASection('x')}catch(e){name=e.name}\
        const c=new Document().createCDATASection('1234');const out=[name,c.data].join('|');";
    assert_eq!(eval(source, "out"), "NotSupportedError|1234");
}

/** insertNode into a collapsed text start splits the text and ends the range after the new node. */
#[test]
fn insert_node_into_a_collapsed_text_start_ends_after_the_node() {
    let source = "const d=document.createElement('div');const t=document.createTextNode('abc');d.append(t);\
        const r=document.createRange();r.setStart(t,1);r.setEnd(t,1);r.insertNode(document.createElement('span'));\
        const out=[r.startContainer===t,r.startOffset,r.endContainer===d,r.endOffset].join();";
    assert_eq!(eval(source, "out"), "true,1,true,2");
}

/** surroundContents refuses a range that partly selects an element. */
#[test]
fn surround_contents_refuses_a_partly_selected_element() {
    let source = "const d=document.createElement('div');d.innerHTML='<p>a</p><p>b</p>';\
        const r=document.createRange();r.setStart(d.firstChild.firstChild,0);r.setEnd(d.lastChild.firstChild,1);\
        let name='none';try{r.surroundContents(document.createElement('span'))}catch(e){name=e.name}\
        const out=name;";
    assert_eq!(eval(source, "out"), "InvalidStateError");
}
