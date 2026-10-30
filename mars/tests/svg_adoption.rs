/** Real Eye fragments preserve SVG names, namespace metadata and Attr identity through V9 adoption. */
use mars::{document, v9};

/** Exercise the app's parser seam instead of V9's simpler standalone test parser. */
fn run(source: &str) -> v9::Execution {
    let document = document::parse("<div id=host></div>").unwrap();
    let host = v9::DocumentHost {
        imports: None,
        canvas: &mars_core::canvas::Unavailable,
        languages: &[],
        fragments: &document::DomFragments,
    };
    let environment = v9::PageEnvironment {
        dom_level: v9::DomLevel::Extended,
        ..Default::default()
    };
    let execution = v9::load_page(v9::PageInput {
        sources: &[source.into()],
        source_keys: &[],
        document: document::script_document(&document, "svg-adoption").unwrap(),
        storage: Default::default(),
        host: &host,
        environment: &environment,
        modules: v9::ModuleInput::NONE,
    })
    .unwrap();
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    document::apply_script_changes(&document, &execution.script_changes).unwrap();
    execution
}

/** Mixed-case SVG nodes retain both their wrappers and their namespace-aware attributes. */
#[test]
fn svg_names_and_attached_attributes_survive_append_remove_and_reinsert() {
    let execution = run(r#"
const ns='http://www.w3.org/2000/svg',link='http://www.w3.org/1999/xlink';
const div=document.createElement('div'),svg=document.createElementNS(ns,'svg'),defs=document.createElementNS(ns,'defs');
svg.appendChild(defs);div.appendChild(svg);
const clip=document.createElementNS(ns,'clipPath'),gradient=document.createElementNS(ns,'linearGradient');
clip.id='clip';clip.setAttributeNS(link,'xlink:href','#shape');const attr=clip.getAttributeNodeNS(link,'href');
defs.appendChild(clip);defs.appendChild(gradient);
for(let pass=0;pass<2;pass++){
 document.body.appendChild(div);
 if(clip.namespaceURI!==ns||clip.localName!=='clipPath'||clip.tagName!=='clipPath'||gradient.localName!=='linearGradient')throw Error('name or namespace');
 if(clip.parentNode!==defs||document.getElementById('clip')!==clip||attr.ownerElement!==clip||clip.getAttributeNodeNS(link,'href')!==attr||attr.value!=='#shape')throw Error('identity');
 if(!div.outerHTML.includes('<clipPath'))throw Error('serialization');
 div.remove();
}
clip.removeAttributeNS(link,'href');if(attr.ownerElement!==null||attr.value!=='#shape')throw Error('detached attribute');
console.log('ok');
"#);
    assert_eq!(execution.effects.console.last().unwrap().text, "ok");
}

/** SVG adjustment does not hide unrelated mixed-case XML structure mismatches. */
#[test]
fn arbitrary_foreign_names_are_not_case_folded_at_adoption() {
    let execution = run(
        "let refused=0;for(const ns of [null,'urn:custom']){const node=document.createElementNS(ns,'MixedCase');try{document.body.appendChild(node)}catch(error){refused++}}console.log(refused);",
    );
    assert_eq!(execution.effects.console.last().unwrap().text, "2");
}
