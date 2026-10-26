/** Arbitrary-width BigInts keep exact values, mathematical conversion, real brands and bounded snapshots. */
mod common;

/** Wide literals and results normalize back to inline storage without changing their language type. */
#[test]
fn literals_and_arithmetic_preserve_values_past_the_inline_boundary() {
    let source = "const literal=0x100000000000000000000000000000000000000000000000001n;const power=2n**200n;const r=[typeof power,literal===power+1n,power-(power-1n),(power*power)/(power),((power-1n)&power)===0n,~(-power)===power-1n].join();";
    assert_eq!(
        common::eval(source, "r"),
        "bigint,true,1,1606938044258990275541962092341162602522202993782792835301376,true,true"
    );
}

/** Normalized multi-limb division agrees with independently computed exact signed quotient and remainder. */
#[test]
fn long_division_preserves_exact_signed_quotients_and_remainders() {
    let source = "const a=6864797660130609714981900799081393217269435300143305409394463459185543183397656052122559640661454554977296311391480858038634354074920814741623610645521616367n,b=231584178474632390847141970017375815706539969331283416892500143212961653408545n;const q=29642774844752946028434172162224104410437116074403691425962264680792430739353472n,r=670549456226177786636641614599172503040056445827141398127n;const out=[];for(const sa of [1n,-1n])for(const sb of [1n,-1n])out.push((a*sa)/(b*sb)===q*sa*sb,(a*sa)%(b*sb)===r*sa);const result=out.every(Boolean);";
    assert_eq!(common::eval(source, "result"), "true");
}

/** Division has truncation signs, while right shifts use infinite sign extension and negative counts reverse direction. */
#[test]
fn signed_shifts_division_and_infinite_bitwise_operations_are_distinct() {
    let source = "const a=-(1n<<200n)-3n,p=1n<<200n;const r=[a>>199n,a/p,a%p,-1n>>(1n<<200n),0n<<(1n<<200n),(p>>-2n)===(p<<2n),((~p)^p)===-1n,((-p)|p)===-p].join();";
    assert_eq!(common::eval(source, "r"), "-3,-1,-3,-1,0,true,true,true");
}

/** Arbitrary asIntN/asUintN widths use mathematical two's-complement truncation. */
#[test]
fn as_int_n_and_as_uint_n_support_wide_widths() {
    let source = "const p=1n<<200n;const r=[BigInt.asUintN(200,-1n)===p-1n,BigInt.asIntN(201,p)===-p,BigInt.asIntN(200,p-1n)===-1n,BigInt.asUintN(129,-1n)===(1n<<129n)-1n,BigInt.asIntN(9007199254740991,p)===p,BigInt.asUintN(0,p)===0n].join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true,true,true");
}

/** Equality and ordering do not silently round neighboring mathematical integers to the same Number. */
#[test]
fn comparisons_and_number_conversion_round_at_the_correct_boundary() {
    let source = "const p=1n<<200n,n=2**200,half=1n<<147n;const r=[p==n,p+1n==n,p-1n<n,p+1n>n,Number(p+half)===n,Number(p+half+1n)===n+2**148,Number(1n<<1024n)===Infinity,170141183460469231731687303715884105727n==2**127,BigInt(1e100).toString()].join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,false,true,true,true,true,true,false,10000000000000000159028911097599180468360808563945281389781327557747838772170381060813469985856815104"
    );
}

/** Every radix round-trips exact immutable magnitudes; decimal conversion does not recurse through Number. */
#[test]
fn wide_radix_formatting_and_parsing_preserve_every_digit() {
    let source = "const value=(1n<<520n)+0x123456789abcdefn;const hex=value.toString(16),binary=value.toString(2);const decimal=(10n**900n).toString();const r=[BigInt('0x'+hex)===value,BigInt('0b'+binary)===value,decimal==='1'+'0'.repeat(900),BigInt('\\uFEFF 0x1'+'0'.repeat(40)+' ')===(1n<<160n)].join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true");
}

/** Sets compare mathematical values, boxed methods keep their brand, and JSON rejects both storage forms. */
#[test]
fn wide_values_have_real_bigint_collection_and_boxing_semantics() {
    let source = "const value=1n<<200n,copy=BigInt(value.toString()),boxed=Object(value);let rejected=false;try{JSON.stringify(boxed)}catch(e){rejected=e instanceof TypeError}const cloned=structuredClone(boxed);const r=[new Set([value,copy]).size,boxed.valueOf()===value,boxed instanceof BigInt,cloned.valueOf()===value,rejected].join();";
    assert_eq!(common::eval(source, "r"), "1,true,true,true,true");
}

/** Element stores wrap modulo 64 bits, while Atomics.store returns the untruncated ToBigInt result. */
#[test]
fn typed_storage_wraps_wide_values_without_changing_atomics_store_results() {
    let source = "const value=(1n<<200n)+7n,array=new BigInt64Array(1);array[0]=value;const stored=Atomics.store(array,0,value);const unsigned=new BigUint64Array(1);unsigned[0]=-value;const r=[array[0],stored===value,unsigned[0],Atomics.add(array,0,value),array[0]].join();";
    assert_eq!(
        common::eval(source, "r"),
        "7,true,18446744073709551609,7,14"
    );
}

/** Actual size limits and invalid operations throw language errors without restoring the obsolete i128 restriction. */
#[test]
fn wide_results_remain_bounded_and_errors_keep_their_actual_types() {
    let source = "const r=[()=>2n**(2n**64n),()=>1n<<(1n<<64n),()=>1n/0n,()=>2n**-1n,()=>1n+1,()=>BigInt('0x-1')].map(fn=>{try{fn();return 'ok'}catch(e){return e.name}}).join();";
    assert_eq!(
        common::eval(source, "r"),
        "RangeError,RangeError,RangeError,RangeError,TypeError,SyntaxError"
    );
}

/** Wide primitives, boxes and captured arithmetic retain their actual limbs across public JSON state transport. */
#[test]
fn wide_values_survive_public_state_transport() {
    let sources=vec!["const value=(1n<<200n)+3n,boxed=Object(value),values=new Set([value]);document.getElementById('go').addEventListener('click',()=>console.log((boxed.valueOf()+1n).toString(16),values.has(BigInt(value.toString()))));".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(
        common::console(&clicked),
        ["100000000000000000000000000000000000000000000000004 true"]
    );
}

/** Persisted magnitudes reject high zero limbs, negative zero and noncanonical wide storage for inline values. */
#[test]
fn malformed_and_noncanonical_bigint_snapshots_are_rejected() {
    for value in [
        serde_json::json!({"B":[false,[1,0]]}),
        serde_json::json!({"B":[true,[]]}),
        serde_json::json!({"B":[false,[1]]}),
    ] {
        assert!(serde_json::from_value::<mars_v9::JsValue>(value).is_err());
    }
}
