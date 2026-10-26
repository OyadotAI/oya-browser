/** Direct label chains share the actual iteration target without crossing block, conditional or function boundaries. */
mod common;

/** Continuing an outer direct label returns to a do-while condition rather than leaving the labelled statement. */
#[test]
fn chained_do_labels_continue_the_same_loop() {
    let source = "let count=0;if(true)outer:inner:do{count++;continue outer;count+=100}while(count<3);const r=count;";
    assert_eq!(common::eval(source, "r"), "3");
}

/** Every label in a direct chain designates one while loop, including the outermost alias. */
#[test]
fn all_chained_while_labels_are_continue_aliases() {
    let source = "let count=0;outer:middle:inner:while(count<3){count++;if(count===1)continue outer;if(count===2)continue middle;continue inner}const r=count;";
    assert_eq!(common::eval(source, "r"), "3");
}

/** A labelled for continuation executes its update and keeps each captured lexical iteration environment distinct. */
#[test]
fn chained_for_labels_preserve_updates_and_lexical_captures() {
    let source = "let updates=0;const readers=[];outer:inner:for(let index=0;index<3;updates++,index++){readers.push(()=>index);continue outer}const r=[updates,readers.map(read=>read()).join(':')].join('|');";
    assert_eq!(common::eval(source, "r"), "3|0:1:2");
}

/** Property enumeration advances after an outer direct continue label instead of stopping at its first key. */
#[test]
fn chained_for_in_labels_advance_the_enumerator() {
    let source = "const keys=[];outer:inner:for(const key in {a:1,b:2,c:3}){keys.push(key);continue outer}const r=keys.join();";
    assert_eq!(common::eval(source, "r"), "a,b,c");
}

/** A same-loop continue must not close a for-of iterator through a mistakenly inserted outer block target. */
#[test]
fn chained_for_of_continue_does_not_close_the_iterator() {
    let source = "let calls=0,closed=0,total=0;const iterable={[Symbol.iterator](){return this},next(){calls++;return {value:calls,done:calls>3}},return(){closed++;return {done:true}}};outer:inner:for(const value of iterable){total+=value;continue outer}const r=[total,calls,closed].join();";
    assert_eq!(common::eval(source, "r"), "6,4,0");
}

/** Label aliases do not bypass a try statement's actual finalizer when control returns to the loop. */
#[test]
fn chained_continue_runs_finally_before_the_next_iteration() {
    let source = "const log=[];outer:inner:for(let index=0;index<3;index++){try{log.push(index);continue outer}finally{log.push('f'+index)}}const r=log.join();";
    assert_eq!(common::eval(source, "r"), "0,f0,1,f1,2,f2");
}

/** Resource cleanup still runs for every iteration when an outer alias continues its owning loop. */
#[test]
fn chained_continue_disposes_iteration_resources() {
    let source = "const log=[];outer:inner:for(let index=0;index<2;index++){using resource={[Symbol.dispose](){log.push('d'+index)}};log.push(index);continue outer}const r=log.join();";
    assert_eq!(common::eval(source, "r"), "0,d0,1,d1");
}

/** Breaking a labelled for-of loop still invokes the iterator's return exactly once. */
#[test]
fn chained_break_closes_the_iterator_once() {
    let source = "let calls=0,closed=0;const seen=[];const iterable={[Symbol.iterator](){return this},next(){calls++;return {value:calls,done:false}},return(){closed++;return {done:true}}};outer:inner:for(const value of iterable){seen.push(value);if(value===2)break outer}const r=[seen.join(':'),calls,closed].join('|');";
    assert_eq!(common::eval(source, "r"), "1:2|2|1");
}

/** The direct-chain rule also reaches an awaited iterator's real loop target through promise suspension. */
#[test]
fn chained_for_await_labels_preserve_async_iteration() {
    let source = "async function f(){const seen=[];outer:inner:for await(const value of [1,2,3]){seen.push(value);continue outer}return seen.join()}f().then(value=>console.log(value));";
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::console(&execution), ["1,2,3"]);
}

/** Blocks, conditionals and function bodies stop continue-label forwarding; duplicate active labels remain syntax errors. */
#[test]
fn noniteration_boundaries_do_not_forward_continue_labels() {
    for source in [
        "outer:if(true)while(false){continue outer}",
        "outer:inner:{while(false){continue outer}}",
        "outer:{inner:for(;;){continue outer}}",
        "outer:inner:while(false){(()=>{continue outer})()}",
        "outer:outer:while(false){}",
    ] {
        assert_eq!(
            mars_v9::check(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
}

/** Non-loop labels remain valid break targets without becoming valid continue targets. */
#[test]
fn chained_block_labels_remain_break_targets() {
    let source = "let count=0;outer:inner:{count++;break outer;count+=10}const r=count;";
    assert_eq!(common::eval(source, "r"), "1");
}

/** Escaped label identifiers are normalized consistently while only the final unescaped loop keyword classifies the chain. */
#[test]
fn escaped_chained_label_identifiers_keep_their_target() {
    let source =
        "let count=0;outer:\\u0069nner:while(count<3){count++;continue outer}const r=count;";
    assert_eq!(common::eval(source, "r"), "3");
}
