/** String.prototype.localeCompare orders text like the browser's collator, not by code unit. */
mod common;
use common::eval;

/** Root collation: letters compare without case first, then lowercase sorts before uppercase. */
#[test]
fn locale_compare_orders_case_after_letters() {
    let source = "const r=['a'.localeCompare('A'),'A'.localeCompare('a'),'a'.localeCompare('b'),'B'.localeCompare('a'),'hello'.localeCompare('hello')].join();";
    assert_eq!(eval(source, "r"), "-1,1,-1,1,0");
}

/** Accents compare after the base letters, and only when the sensitivity keeps them. */
#[test]
fn locale_compare_orders_accents_after_base_letters() {
    let source = "const r=['\u{e4}'.localeCompare('b'),'a'.localeCompare('\u{e4}'),'r\u{e9}sum\u{e9}'.localeCompare('resume'),'\u{e9}'.localeCompare('e',undefined,{sensitivity:'base'}),'a'.localeCompare('A',undefined,{sensitivity:'base'}),'a'.localeCompare('A',undefined,{sensitivity:'case'})].join();";
    assert_eq!(eval(source, "r"), "-1,-1,1,0,0,-1");
}

/** The numeric option compares digit runs by value, and sort callbacks see the same order. */
#[test]
fn locale_compare_numeric_option_and_sort_agree_with_collator() {
    let source = "const names=['b','A','a','B','10','9'];const r=[names.slice().sort((x,y)=>x.localeCompare(y)).join(),'10'.localeCompare('9',undefined,{numeric:true}),names.slice().sort(new Intl.Collator().compare).join()].join('|');";
    assert_eq!(eval(source, "r"), "10,9,a,A,b,B|1|10,9,a,A,b,B");
}
