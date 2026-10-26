/** Explicit shell seeds select V8's real MurmurHash3-initialized xorshift128+ stream per creation context. */
use crate::protocol::{EngineError, EngineResult};
use crate::v9::vm::{Res, Vm};
use serde::{Deserialize, Serialize};

/** MurmurHash3's fixed avalanche multipliers are part of the pinned generator algorithm. */
const HASH_FIRST: u64 = 0xff51_afd7_ed55_8ccd;
/** The second avalanche multiplication mixes the already shifted and multiplied seed. */
const HASH_SECOND: u64 = 0xc4ce_b9fe_1a85_ec53;
/** MurmurHash3's 64-bit finalizer uses three right shifts by one fixed count. */
const HASH_SHIFT: u32 = 33;
/** The xorshift128+ transition uses its specified left shift. */
const XOR_LEFT: u32 = 23;
/** The first right shift mixes the updated first word. */
const XOR_FIRST: u32 = 17;
/** The second right shift mixes the other word. */
const XOR_SECOND: u32 = 26;
/** Dropping eleven low bits leaves exactly representable binary64 integer outputs. */
const DOUBLE_SHIFT: u32 = 11;
/** Dividing by two to the fifty-third maps all retained integers into [0,1). */
const DOUBLE_SCALE: f64 = 9_007_199_254_740_992.0;

/** Two fixed words are the entire mutable generator state; no generated-output table is stored. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct State {
    /** The older word becomes the next transition's second operand. */
    first: u64,
    /** The newer word becomes the next transition's first operand. */
    second: u64,
}

/** Absence preserves Mars's existing production generator; an explicit zero asks for native entropy. */
pub(in crate::v9) fn next(vm: &mut Vm) -> Res<Option<f64>> {
    let Some(seed) = vm.page.environment.conformance_random_seed else {
        return Ok(None);
    };
    let bank = vm.active_realm;
    if !vm.page.state.math_random.contains_key(&bank) {
        let state = initialize(seed)?;
        vm.page.state.math_random.insert(bank, state);
    }
    let state = vm
        .page
        .state
        .math_random
        .get_mut(&bank)
        .expect("initialized random state");
    Ok(Some(draw(state)))
}

/** Each creation context restarts the same requested sequence, independently of every other context. */
fn initialize(seed: i32) -> EngineResult<State> {
    let seed = if seed == 0 {
        entropy()?
    } else {
        seed as i64 as u64
    };
    Ok(State {
        first: hash(seed),
        second: hash(!seed),
    })
}

/** A zero seed has the shell's actual nondeterministic meaning rather than a fabricated fixed seed. */
fn entropy() -> EngineResult<u64> {
    use ring::rand::SecureRandom;
    let mut bytes = [0; std::mem::size_of::<u64>()];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| {
            EngineError::unsupported("The operating system random source is unavailable to V9.")
        })?;
    Ok(u64::from_ne_bytes(bytes))
}

/** This is the pinned MathRandom initializer, independent of V8's separate isolate RNG initializer. */
fn hash(mut value: u64) -> u64 {
    value = (value ^ (value >> HASH_SHIFT)).wrapping_mul(HASH_FIRST);
    value = (value ^ (value >> HASH_SHIFT)).wrapping_mul(HASH_SECOND);
    value ^ (value >> HASH_SHIFT)
}

/** Wrapping arithmetic implements the native unsigned transition and exact top-53 conversion. */
fn draw(state: &mut State) -> f64 {
    let mut first = state.first;
    let second = state.second;
    state.first = second;
    first ^= first << XOR_LEFT;
    first ^= first >> XOR_FIRST;
    state.second = first ^ second ^ (second >> XOR_SECOND);
    (second.wrapping_add(state.second) >> DOUBLE_SHIFT) as f64 / DOUBLE_SCALE
}

/** Saved data cannot create an all-zero absorbing stream or extra contexts outside the existing realm bound. */
pub(in crate::v9) fn validate(vm: &Vm) -> EngineResult<()> {
    let states = &vm.page.state.math_random;
    if states.len() > vm.realms.contexts.len()
        || states.iter().any(|(bank, state)| {
            *bank as usize >= vm.realms.contexts.len() || state.first == 0 && state.second == 0
        })
    {
        return Err(EngineError::invalid("Invalid V9 Math.random state."));
    }
    Ok(())
}
