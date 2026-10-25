/** Frames: same-origin iframe documents, Document::open/write/close and createHTMLDocument,
ported from Blink's HTMLFrameOwnerElement, Document and DOMImplementation. */
mod common;
use common::eval;

/** The html5lib serializer test.js runs, over any document. */
const TREE: &str = "function tree(n,d){var p=d>0?' '.repeat(2*d-1):'',s='';\
    if(n.nodeType==10)s='|'+p+'<!DOCTYPE '+n.name+'>\\n';\
    else if(n.nodeType==8)s='|'+p+'<!-- '+n.nodeValue+' -->\\n';\
    else if(n.nodeType==3)s='|'+p+'\"'+n.nodeValue+'\"\\n';\
    else if(n.nodeType==1)s='|'+p+'<'+n.localName+'>\\n';\
    else if(n.nodeType==9)s='#document\\n';\
    for(var c of n.childNodes)s+=tree(c,d+1);return s}";

/** An about:blank iframe has the initial document: html, head and body. */
#[test]
fn blank_iframe_has_a_same_origin_document() {
    let source = format!(
        "{TREE};var f=document.createElement('iframe');document.body.appendChild(f);\
         var d=f.contentDocument;\
         const r=[tree(d,0),d.nodeType,f.contentWindow.document===d,d.body.nodeName,\
         d.documentElement.ownerDocument===d,d.body.parentNode.parentNode===d].join(',');"
    );
    assert_eq!(
        eval(&source, "r"),
        "#document\n| <html>\n|   <head>\n|   <body>\n,9,true,BODY,true,true"
    );
}

/** A frame not yet connected, or one showing another origin, has no reachable document. */
#[test]
fn other_frames_have_no_document() {
    let source = "var a=document.createElement('iframe');var b=document.createElement('iframe');\
        b.src='https://elsewhere.example/';document.body.appendChild(b);\
        const r=[a.contentDocument,b.contentDocument,b.contentWindow].join(',');";
    assert_eq!(eval(source, "r"), ",,");
}

/** open, write and close replace the document with the written markup, as the parser's
initial, in-head and in-body modes place it, then fire the frame's load event. */
#[test]
fn written_markup_becomes_the_document() {
    let source = format!(
        "{TREE};var f=document.createElement('iframe');document.body.appendChild(f);var loads=0;\
         f.onload=function(){{loads++}};var d=f.contentDocument;d.open();\
         d.write('<!DOCTYPE html><!--c--><title>T</title>');d.write('<p>Hi');d.close();\
         const r=tree(d,0)+loads+','+d.doctype.name+','+d.title;"
    );
    assert_eq!(
        eval(&source, "r"),
        "#document\n| <!DOCTYPE html>\n| <!-- c -->\n| <html>\n|   <head>\n|     <title>\n|       \"T\"\n|   <body>\n|     <p>\n|       \"Hi\"\n1,html,T"
    );
}

/** Markup written one character at a time parses as the whole. */
#[test]
fn single_character_writes_parse_as_one() {
    let source = format!(
        "{TREE};var f=document.createElement('iframe');document.body.appendChild(f);\
         var d=f.contentDocument;d.open();for(var ch of 'Test<b>x</b>')d.write(ch);d.close();\
         const r=tree(d,0);"
    );
    assert_eq!(
        eval(&source, "r"),
        "#document\n| <html>\n|   <head>\n|   <body>\n|     \"Test\"\n|     <b>\n|       \"x\"\n"
    );
}

/** createHTMLDocument gives a doctype, then html with a titled head and a body. */
#[test]
fn create_html_document_has_doctype_head_and_body() {
    let source = format!(
        "{TREE};var d=document.implementation.createHTMLDocument('Hey');\
         const r=tree(d,0)+[d.doctype.nodeName,d.head.firstChild.textContent,\
         d.getElementsByTagName('title').length,d.querySelector('body')===d.body].join(',');"
    );
    assert_eq!(
        eval(&source, "r"),
        "#document\n| <!DOCTYPE html>\n| <html>\n|   <head>\n|     <title>\n|       \"Hey\"\n|   <body>\nhtml,Hey,1,true"
    );
}
