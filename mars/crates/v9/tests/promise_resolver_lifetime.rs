/** Promise resolving functions share one once-only record while releasing settled targets. */
mod common;

/** Clearing the target after claiming must not allow the sibling resolver to settle again. */
#[test]
fn resolving_functions_claim_once_across_thenable_adoption() {
    let source = "let resolve,reject,r='pending';const p=new Promise((a,b)=>{resolve=a;reject=b});resolve({then(a,b){a(7);b(8);a(9)}});reject(10);resolve(11);p.then(x=>r='ok:'+x,e=>r='error:'+e);";
    assert_eq!(common::eval(source, "r"), "ok:7");
}
