/** SVG interface completion must never install readonly SVG members on HTML element prototypes. */
use crate::common;

/** Polymer installs a transform method on a prototype derived from HTMLElement before making instances. */
#[test]
fn html_mixin_can_install_transform_without_inheriting_svg_readonly_properties() {
    let execution = common::load(
        "'use strict';class Legacy extends HTMLElement{};Legacy.prototype.transform=function(value,target){target=target||this;target.style.webkitTransform=value;target.style.transform=value};const target=document.createElement('div');Legacy.prototype.transform.call(target,'translateX(10px)');target.transform=42;const descriptor=Object.getOwnPropertyDescriptor(SVGGraphicsElement.prototype,'transform');const result=[!('transform' in HTMLElement.prototype),!('ownerSVGElement' in HTMLElement.prototype),target.transform,target.style.transform,typeof descriptor.get,descriptor.set===undefined].join('|');",
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert!(
        execution.load_failure.is_none(),
        "{:?}",
        execution.load_failure
    );
    assert_eq!(
        common::global(&execution, "result"),
        "true|true|42|translateX(10px)|function|true"
    );
}

/** Namespace-aware construction chooses SVG's existing specialized interface, retaining native style/events. */
#[test]
fn svg_root_and_path_use_svg_prototypes_and_native_style_bindings() {
    let execution = common::load(
        "const namespace='http://www.w3.org/2000/svg',svg=document.createElementNS(namespace,'svg'),path=document.createElementNS(namespace,'path');svg.appendChild(path);path.style.transform='translateX(4px)';let events=0;path.addEventListener('example',()=>events++);path.dispatchEvent(new Event('example'));const result=[Object.getPrototypeOf(SVGElement.prototype)===Element.prototype,Object.getPrototypeOf(SVGGraphicsElement.prototype)===SVGElement.prototype,Object.getPrototypeOf(SVGSVGElement.prototype)===SVGGraphicsElement.prototype,svg instanceof SVGSVGElement,path instanceof SVGPathElement,path instanceof SVGGraphicsElement,path instanceof SVGElement,!(path instanceof HTMLElement),path.style===path.style,path.style.transform,events].join('|');",
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert!(
        execution.load_failure.is_none(),
        "{:?}",
        execution.load_failure
    );
    assert_eq!(
        common::global(&execution, "result"),
        "true|true|true|true|true|true|true|true|true|translateX(4px)|1"
    );
}

/** A tag string alone cannot turn HTML or arbitrary XML namespace elements into SVG wrappers. */
#[test]
fn svg_tag_spelling_does_not_override_the_explicit_namespace() {
    let execution = common::load(
        "const html=document.createElement('svg'),foreign=document.createElementNS('urn:example','path'),unknown=document.createElementNS('http://www.w3.org/2000/svg','not-an-svg-tag');const result=[html instanceof HTMLElement,!(html instanceof SVGElement),foreign instanceof Element,!(foreign instanceof HTMLElement),!(foreign instanceof SVGElement),unknown instanceof SVGElement,!(unknown instanceof SVGGraphicsElement)].join('|');",
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert!(
        execution.load_failure.is_none(),
        "{:?}",
        execution.load_failure
    );
    assert_eq!(
        common::global(&execution, "result"),
        "true|true|true|true|true|true|true"
    );
}
