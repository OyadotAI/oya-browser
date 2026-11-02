/** Named evaluation distinguishes real properties from prototype setters and preserves method/accessor key names. */
use crate::common;

/** Computed string, number and symbol keys name every ordinary, async and generator method. */
#[test]
fn computed_methods_use_the_actual_property_key() {
    let source = "const sym=Symbol('value');const o={*['gen'](){},async ['task'](){},async *[sym](){},[42](){},get ['x'](){},set ['x'](v){}};const d=Object.getOwnPropertyDescriptor(o,'x');const r=[o.gen.name,o.task.name,o[sym].name,o[42].name,d.get.name,d.set.name].join();";
    assert_eq!(common::eval(source, "r"), "gen,task,[value],42,get x,set x");
}

/** Replacing a function's original name preserves its key position, while deletion followed by creation moves it. */
#[test]
fn function_metadata_redefinition_preserves_property_order() {
    let source = "class C{static name(){}}const out=[Object.getOwnPropertyNames(C).join('|')];function f(){}const original=Object.getOwnPropertyNames(f);Object.defineProperty(f,'name',{get(){return 'x'}});out.push(Object.getOwnPropertyNames(f).join()===original.join());delete f.name;Object.defineProperty(f,'name',{value:'new'});const final=Object.getOwnPropertyNames(f);out.push(final.at(-1)==='name',final.slice(0,-1).join()===original.filter(x=>x!=='name').join());const r=out.join(',');";
    assert_eq!(
        common::eval(source, "r"),
        "length|name|prototype,true,true,true"
    );
}

/** The __proto__ colon syntax changes inheritance and does not supply a function's inferred name. */
#[test]
fn prototype_setters_do_not_infer_function_names() {
    let source = "const a={__proto__:function(){}};const b={'__proto__':()=>{}};const c={['__proto__']:function(){}};const r=[Object.getPrototypeOf(a).name,Object.getPrototypeOf(b).name,c.__proto__.name].join();";
    assert_eq!(common::eval(source, "r"), ",,__proto__");
}

/** Date's GMT spelling is the same function object as UTC and retains its canonical name. */
#[test]
fn date_gmt_alias_preserves_function_identity() {
    let source = "const r=[Date.prototype.toGMTString===Date.prototype.toUTCString,Date.prototype.toGMTString.name,Date.prototype.setYear.name,Date.prototype.setYear.length].join();";
    assert_eq!(common::eval(source, "r"), "true,toUTCString,setYear,1");
}

/** Legacy setYear preserves captured fields even when year conversion mutates the receiver. */
#[test]
fn legacy_year_setter_uses_the_original_time() {
    let source = "const d=new Date(Date.UTC(2020,5,12,3,4));d.setYear({valueOf(){d.setTime(0);return 99}});const bad=new Date(NaN);bad.setYear(5);const r=[d.getFullYear(),d.getMonth(),d.getDate(),d.getHours(),d.getMinutes(),bad.getFullYear(),bad.getMonth(),bad.getDate()].join();";
    assert_eq!(common::eval(source, "r"), "1999,5,12,3,4,1905,0,1");
}
