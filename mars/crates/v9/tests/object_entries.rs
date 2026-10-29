/** Object.values and Object.entries check each key's descriptor right before reading its value. */
mod common;

/** A getter that deletes a later key or hides it stops that key from appearing. */
#[test]
fn getters_that_remove_or_hide_later_keys_are_seen() {
    let source = "const a={get x(){delete this.y;return 1},y:2};\
        const b={get x(){Object.defineProperty(this,'y',{enumerable:false});return 1},y:2};\
        const r=[JSON.stringify(Object.entries(a)),JSON.stringify(Object.values(b))].join('|');";
    assert_eq!(common::eval(source, "r"), "[[\"x\",1]]|[1]");
}

/** A key a getter adds is not visited, since the key list is taken before any value is read. */
#[test]
fn keys_added_by_a_getter_are_not_visited() {
    let source =
        "const o={get x(){this.z=3;return 1},y:2};const r=JSON.stringify(Object.entries(o));";
    assert_eq!(common::eval(source, "r"), "[[\"x\",1],[\"y\",2]]");
}

/** A proxy sees getOwnPropertyDescriptor and get interleaved for each key. */
#[test]
fn proxies_observe_interleaved_operations() {
    let source = "let log='';const p=new Proxy({a:0,b:0},{\
        ownKeys(t){log+='|keys';return Reflect.ownKeys(t)},\
        getOwnPropertyDescriptor(t,k){log+='|desc:'+k;return Reflect.getOwnPropertyDescriptor(t,k)},\
        get(t,k){log+='|get:'+k;return t[k]}});Object.entries(p);const r=log;";
    assert_eq!(common::eval(source, "r"), "|keys|desc:a|get:a|desc:b|get:b");
}
