/** Form pipelines and callback failures cross the real network, document and V9 service boundary. */
use super::*;

/** Current native edits feed ordinary String/Array methods after each serialized event. */
#[test]
fn delimited_names_use_saved_methods_and_callbacks_across_native_events() {
    let body = r#"<input id=name><p id=out>initial</p><p id=count>0</p><button id=run type=button>Run</button><script>
const input=document.getElementById('name');const output=document.getElementById('out');const counter=document.getElementById('count');const button=document.getElementById('run');
const trim=''.trim;const map=[].map;const join=[].join;let count=0;
button.addEventListener('click',function(){const names=input.value.split(',').map(name=>name.trim()).filter(name=>name);if(trim!==''.trim||map!==[].map||join!==[].join){missing();}output.textContent=names.join(', ');counter.textContent=++count;});
</script>"#;
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let mut page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    for (count, entered, expected) in [
        (1, " Ada, ,Lin ", "Ada, Lin"),
        (2, " , ,, ", ""),
        (3, " Rust 🐙 ,  Ada Lin ", "Rust 🐙, Ada Lin"),
    ] {
        let mut params = target(&page, "name");
        params["text"] = json!(entered);
        call(&mut runtime, "page.type", params).unwrap();
        page = call(&mut runtime, "page.inspect", json!({})).unwrap();
        page = call(&mut runtime, "page.click", target(&page, "run")).unwrap();
        assert_eq!(text(&page, "out"), expected);
        assert_eq!(text(&page, "count"), count.to_string());
        let input = page["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["html_id"] == "name")
            .unwrap();
        assert_eq!(input["value"], entered);
    }
}

/** Callback and late host failures keep the source-array writes, cells, Storage and text made before the throw. */
#[test]
fn array_callback_effects_before_a_throw_commit_through_real_services() {
    let body = r#"<p id=out>initial</p><button id=good onclick='run(false)'>Good</button><button id=late onclick='run(true)'>Late failure</button><button id=callback onclick='bad()'>Callback failure</button><button id=probe onclick='probe()'>Probe</button><script>
const values=[1,2,3];let count=0;
function run(fail){const result=values.map(function(value,index,source){count++;if(index===0){source[1]+=1;}return value*2;});window.localStorage.setItem('count',count);document.getElementById('out').textContent=result.join(',');if(fail){document.getElementById('missing').textContent='fail';}}
function bad(){values.filter(function(value,index,source){count++;source[1]=90;window.localStorage.setItem('count',count);document.getElementById('out').textContent='partial';if(index===1){missing();}return true;});}
function probe(){document.getElementById('out').textContent=count+' '+values.join(',')+' '+window.localStorage.getItem('count');}
</script>"#;
    let fixture = fixture(body);
    let mut runtime = runtime(&fixture.url);
    let page = call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "2,6,6");
    let page = call(&mut runtime, "page.click", target(&page, "callback")).unwrap();
    assert_eq!(text(&page, "out"), "partial");
    let page = call(&mut runtime, "page.click", target(&page, "late")).unwrap();
    assert_eq!(text(&page, "out"), "2,182,6");
    let page = call(&mut runtime, "page.click", target(&page, "probe")).unwrap();
    assert_eq!(text(&page, "out"), "8 1,91,3 8");
    let page = call(&mut runtime, "page.click", target(&page, "good")).unwrap();
    assert_eq!(text(&page, "out"), "2,184,6");
}

/** String/Array capabilities advertise implemented methods while retaining full-JavaScript unavailability. */
#[test]
fn native_capabilities_expose_bounded_string_and_array_processing() {
    let fixture = fixture("<p>capabilities</p>");
    let mut runtime = runtime(&fixture.url);
    let capabilities = call(&mut runtime, "runtime.capabilities", json!({})).unwrap();
    let v9 = capabilities["subsets"]["v9"].as_str().unwrap();
    assert!(v9.contains("UTF-16 string length and indexed reads"));
    assert!(v9.contains("string trim and literal-separator split"));
    assert!(v9.contains("array map, filter and join"));
    assert!(
        capabilities["unavailable"]
            .as_array()
            .unwrap()
            .contains(&json!("full_javascript"))
    );
}
