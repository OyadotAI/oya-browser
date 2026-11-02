# C3: value representation, operand stack and property keys

Status: design note for maintainer review. No engine code has changed. Measured on 2026-10-28
at commit d6f7ce86 plus the working tree, on an Apple Silicon Mac, release build.

## 1. What was measured

A temporary test under `crates/v9/tests/` (now deleted) timed `common::eval` with
`std::time::Instant`, best of three runs, one million iterations per row. Each row runs inside a
function so locals are registers: `(function(){ setup; let x=0; for(let i=0;i<N;i++){ body } })()`.
"Over x=a" subtracts a loop whose body is `x=a;`, so it is the cost of the operation itself.

Browser configuration (`cargo test --release --offline -p mars-v9`):

| Row | Body | ns per iteration | ns over `x=a` |
|:---|:---|---:|---:|
| empty loop | (none) | 95 | |
| baseline | `x=a` | 144 | 0 |
| arithmetic | `x=a+b` | 156 | 12 |
| call | `x=f()` | 286 | 141 |
| property read (own) | `x=o.p` | 176 | 32 |
| property read (prototype method) | `x=o.m` | 214 | 70 |
| new | `x=new C()` | 989 | 845 |
| indexed read | `x=arr[j]` | 182 | 38 |
| string value push | `x=s` | 158 | 14 |

Conformance configuration (`--features conformance`):

| Row | ns per iteration | ns over `x=a` |
|:---|---:|---:|
| empty loop | 109 | |
| baseline `x=a` | 133 | 0 |
| arithmetic | 156 | 24 |
| call | 273 | 140 |
| property read (own) | 679 | 546 |
| property read (prototype method) | 876 | 744 |
| new | 705 | 572 |
| indexed read | 309 | 176 |
| string value push | 140 | 8 |

The same rows written as top-level script code (bindings are global lexical bindings, not
registers), browser configuration: empty 85, `x=a` 160, arithmetic 186, call 267, property 170,
new 502, indexed `arr[i&7]` 213 ns per iteration. The conformance run of this variant did not
report and was not repeated.

The plan's figures (arithmetic 200, call 370, property 150, new 900, indexed 500) match these
per-iteration numbers in size except indexed read, which I measured at 180 to 310, not 500. The
plan's indexed row probably read through a global or a non-dense array; whoever runs the gate
should pin the exact benchmark script first (section 7).

The important reading of the table: an empty `for` loop is about 10 bytecodes and costs 85 to
110 ns, so **dispatch alone is about 9 to 10 ns per instruction**. V8's Ignition is about 1 to 2
ns. Most rows are dominated by the instruction count times that per-instruction cost, not by
the operation they name. `new` is the exception: it is slow for its own reasons.

Sizes (`std::mem::size_of`, from the temporary test):

| Type | Browser | Conformance |
|:---|---:|---:|
| `JsValue` | 32 | 32 |
| `Option<JsValue>` | 32 | 32 |
| `JsString` | 32 | 24 |
| `VmState` | 1872 | 2272 |

Derived from the definitions, not measured (the types are crate-private): `Key` is 40 bytes (a
`JsString` plus the tag), `Prop` is 40 (a 32-byte `Slot` plus attributes, padded), `Op` is 16
(`Op::Number(f64)` plus the tag).

`JsValue` is 32 bytes, not 16, because of `BigInt(i128)`: an i128 is 16-byte aligned, which forces
the enum to 32. Every push, pop, peek, register read, environment slot and property slot moves
32 bytes and runs a non-trivial `drop_in_place` because `String` and `BigIntWide` own `Arc`s.

## 2. Where the time goes

Profiled with macOS `sample` (no flamegraph or samply was installed) on the browser
configuration. Percentages are of the samples in the V9 thread. The release build inlines
`execute` into `run`, so "dispatch prelude" below is `run`'s self time.

**Arithmetic (`x=a+b`)**, 2170 samples: `peek` 12%, `drop_in_place<JsValue>` 11%, `push` 10%,
`replace_pair` 5%, `JsValue::eq` 3%, `pop` 3%, `clone` 1.5%. Together **45% is moving
JsValues on and off the operand stack**. `op` itself 36%, the dispatch prelude 15%.

**Call (`x=f()`)**, 2690 samples: `call_op` and below 18%, `drop_in_place<JsValue>` 11%, `peek`
11%, `push` 6%, `do_return` 6%, `Arc::make_mut` 4% (environment writes through the persistent
trie), `env_slot` 3%, `run` prelude 14%.

**New (`x=new C()`)**, 3312 samples: **`prototype_for` is 26%**. It calls
`Key::from("prototype")` on every construction, which allocates and encodes a fresh `JsString`
from UTF-8 (`from_units`, `malloc`, `realloc`, `free`), then does a full non-cached `[[Get]]`
comparing string contents. `alloc` (inserting the new object into the persistent trie, with
`Arc::make_mut` on each of six levels and node copies) is 12%. The collector (`Trie::retain`)
is 4%. JsValue drop, peek and push are another 15%.

**Own property read (`x=o.p`)**, 1371 samples: `peek` 15%, `drop_in_place` 13%, `push` 7%,
`pop` 5%, `load_named` 4%. The inline cache hits; the cost is the values around it.

**Prototype method read (`x=o.m`)**, 2202 samples: `load_named` 17% inclusive, of which
`get_from` 11% and `Props::position` 3% (linear key scan comparing `JsString` contents). The
inline cache only records own data properties, so every prototype read misses. `Arc::make_mut`
5% and `env_slot` 5% come from environment slots living in the trie.

**Indexed read (`x=arr[j]`)**, 1488 samples: `drop_in_place` 14%, `peek` 13%, `push` 7%, `pop`
5%, `fast_element` only 1.3%. Again the element read is cheap and the stack traffic is not.

**Conformance configuration, property read.** The 546 ns is not value representation. Every
`load_named` first calls `is_shared_object` (three lookups in conformance side tables) and
`guard_realm_global`, and every string constant goes through `isolate_code_constant`, which
hashes the whole string with SipHash on a cache miss. It belongs to C3's gate because the gate
covers both configurations, but its fix is to put those checks behind a per-object flag, not a
new value type.

### The three biggest costs

1. **JsValue traffic on the operand stack.** 32-byte values, cloned by `peek` and dropped by
   `pop` and `drop_n`, with a `drop_in_place` call that is not inlined. 35 to 45% of every row
   except `new`. `push` also calls `track_string` on every string value, which downgrades an
   `Arc` and inserts into a hash map (`StringBudget::register`).
2. **Instruction count times a 9 to 10 ns dispatch.** `x=a+b` is five instructions (`GetReg`,
   `GetReg`, `Add`, `SetReg`, `Pop`). Each runs the prelude in `execute`: `select_frame_realm`,
   the budget decrement and stride check, `collect_if_due`, `frames.last_mut()` and a bounds
   checked `ops.get(pc)`.
3. **String-keyed lookups on the slow paths.** `Key::from("prototype")` allocating per `new`,
   prototype reads that never hit the inline cache and compare key contents, shape transitions
   that hash full key text under a `Mutex` per added property, global bindings looked up by text.
   Closely tied to this: environments and objects live in a 64-ary persistent trie, so every
   write runs `Arc::make_mut` on six levels.

## 3. Proposed design

### 3.1 Values: an 8-byte `Copy` value inside the VM, `JsValue` at the edges

`JsValue` stays the public type. It is used by tests, by `VmState`'s wire format and by the
platform layer, and nothing outside `crates/v9` matches on it. Inside the VM, a new crate-private
type replaces it in the four hot places: `Frame::stack`, `Frame::regs`, `Env::slots` and
`Slot::Data`.

```rust
/// One JavaScript value in 8 bytes. Copy: no clone, no drop.
#[derive(Clone, Copy, PartialEq)]
pub(in crate::v9) struct Val(u64);
```

Encoding (NaN boxing). Any f64 that is not a NaN with the top 13 bits all set is stored as its
bits. Canonical NaN is stored as one fixed pattern. The remaining quiet-NaN space holds a 3-bit
tag and a 48-bit payload:

| Tag | Payload | Replaces |
|:---|:---|:---|
| 0 | 0 undefined, 1 null, 2 false, 3 true, 4 hole | `Undefined`, `Null`, `Boolean`, `Hole` |
| 1 | `ObjectId` (u32) | `Object` |
| 2 | symbol id (u32) | `Symbol` |
| 3 | string handle (u32) | `String` |
| 4 | bigint handle (u32), or an inline i32 with a flag bit | `BigInt`, `BigIntWide` |
| 5 | Smi i32 (conformance only) | `Smi` |
| 6 | heap-number handle (conformance only) | `SharedNumber`, `LocalNumber` |

Strings and wide bigints go **behind a handle**: an index into a per-VM table
`Heap::strings: Trie<JsString>` (and `Heap::bigints`). Using the same persistent trie as objects
keeps the existing rule that a step runs on an O(1) copy of the heap and a failed step drops its
copy without touching the committed one. A handle is reclaimed by the collector, not by
reference counting, so `Val` is `Copy`, and `peek`, `pop`, `drop_n`, register reads and slot
writes become 8-byte loads and stores with no drop.

Small bigints keep an inline fast path, but i128 no longer fits; values that fit in i32 stay
inline and everything else takes a handle. BigInt arithmetic is rare on real pages.

Why handles and not tagged `Arc` pointers: a tagged pointer keeps the atomic reference count on
every string copy, needs hand-written unsafe `Clone` and `Drop`, and cannot be `Copy`. A handle
is plain data, which also keeps the "no native handles in state" rule from `value.rs`'s header.

String budget accounting moves from `push` to the table: units are counted when a string enters
the table and released when the collector frees it. That removes `track_string` from every push.

### 3.2 Register operands for the hot instructions

The frame already has a register file (`Frame::regs`) for non-captured locals. The interpreter
stays a stack machine. The compiler adds register-operand forms for the cases that dominate
real code, chosen when every operand is a plain, already-initialized register (no TDZ check,
not captured):

- `BinRR(kind, dst, a, b)` and `BinRK(kind, dst, a, constant)` for `+ - * / % & | ^ << >> >>>`
  and the comparisons. `x=a+b` goes from 5 instructions to 1.
- `JumpIfCmpRR(kind, a, b, target)` and `JumpIfCmpRK` for loop tests. `i<N` plus `JumpIfFalse`
  goes from 4 instructions to 1.
- `IncR(slot)` for `i++` and `i+=1` used as a statement (today about 6 instructions).
- `GetPropR(dst, obj, site)` and `GetElemRR(dst, obj, index)`.
- `MovR(dst, src)` for `x=a`.

Each new op falls back to the existing generic path (`add`, `less_than`, `load_named`) by
reading its operands into locals and calling the same functions, so semantics live in one place.
`Op` stays 16 bytes: a kind byte plus three u16 operands fits.

The dispatch prelude also shrinks: the realm only changes on frame entry and exit, so
`select_frame_realm` moves to `call_closure`, `do_return` and `unwind`; `collect_if_due` folds
into the stride check (a due collection sets `remaining` to the next stride boundary); `execute`
caches the code's op slice and `pc` in locals between calls into `op`. This is not one of the
three named parts, but the empty loop is the floor under every row and it is part of halving it.

### 3.3 Interned property keys

`Key::Str(JsString)` becomes `Key::Atom(Atom)` for every string key that is interned, where
`Atom` is a u32 into a process-wide atom table (thread-local like the shape root, or a sharded
`RwLock`; shapes are already process-wide).

- The compiler interns every named-key constant once per `Code`: `Code::atoms: Vec<Atom>`
  beside `Code::strings`. `constant_key` becomes an index, with no clone and no hashing.
- `Shape::keys: Vec<Key>` compares atoms by integer; `Shape::index` and `transitions` hash a u32.
- Well-known names the engine itself uses (`prototype`, `constructor`, `length`, `then`, `next`,
  `value`, `done`, `toString`, `valueOf`) are pre-interned constants. `prototype_for` then does
  no allocation at all, and gets an inline cache of its own on the function's shape.
- Dynamic keys (`o[k]`) are interned at `to_key` when they are at most 64 units. Longer ones stay
  `Key::Str`. Canonical rule: **a string that is in the atom table is never represented as
  `Key::Str`**, so equality stays structural. Array-index keys keep their own path.
- `Key::Private` stays as it is (private names are unique per class evaluation, see
  `private_for_evaluation`).
- Unbounded growth: a page can mint millions of distinct short keys. The table is bounded
  (count and units, under `JS_ENVIRONMENT_UNITS`-style limits); past the bound new keys stay
  `Key::Str`. Collection of unused atoms is out of scope for the first version.

The inline cache also learns prototype hits: a load site records (receiver shape, holder object,
holder shape, slot) for a method found one level up, validated by the two shape ids. That turns
`o.m` into a hit.

## 4. Migration order

Every step is a separate change that keeps all V9 suites green in both configurations, and keeps
mjsunit and test262 at or above their current numbers. Each step reruns the benchmark of
section 7 and records the rows.

1. **Pin the benchmark.** Check in `crates/v9/tests/perf_ops.rs` as an `#[ignore]` test with the
   five rows above (plus the empty loop and baseline), both in a function and at top level. No
   engine change. This is the yardstick for every later step.
2. **Stop allocating the well-known keys.** A small static set of pre-built `Key`s for
   `prototype`, `constructor`, `length` and friends, used by `prototype_for` and the other
   `Key::from("...")` call sites in hot paths. Expected: `new` drops by about a quarter. Lowest
   risk, no representation change, and it proves the measurement loop.
3. **Dispatch prelude.** Realm selection at frame boundaries only, `collect_if_due` folded into
   the stride check, cached op slice. Expected: 2 to 3 ns off every instruction.
4. **Register-operand ops** (3.2), compiler first behind a switch so both forms can be compared
   on the same suite, then on by default. The generic paths stay as the fallback.
5. **Atoms** (3.3): `Key::Atom` added beside `Key::Str`, the canonical rule enforced in `to_key`
   and `Key::from`, then compiler constants switched, then shapes. Wire format unchanged: atoms
   serialize as their text and are re-interned on decode.
6. **Prototype inline cache** on top of atoms and shapes.
7. **`Val` in `Frame::stack` and `Frame::regs` only**, converting to and from `JsValue` at
   `push` of a string, at native calls (`Call::args` stays `&[JsValue]` at first) and at frame
   save. Strings get the handle table here. Suspended generator frames serialize their stack as
   `JsValue`, so the wire form does not change.
8. **`Val` in `Env::slots` and `Slot::Data`**, again converting at the serde boundary.
9. **Natives move to `Val`** builtin by builtin, starting with the ones the profile of a real
   page names (array, string, object, JSON). `JsValue` remains for the public API and tests.

Steps 2, 3 and 4 are worth doing even if the maintainer stops there; 7 to 9 are the expensive
part (about 280 files in `crates/v9/src` name `JsValue::`, with 4859 uses in total).

## 5. Risks

**Garbage collection.** Handles make strings collector-managed, so a string held only in a Rust
local across an allocation can be freed. Today natives pin what they allocate (`Vm::pins`,
`pinning`), and `alloc` only collects inside a native. The same pinning must cover string
handles: every handle created while `pinning` is set is pinned until the native returns, and the
interpreter's own locals are only held between instructions, where nothing collects. A debug
build should carry a generation in unused handle bits and assert on stale use. This is the risk
most likely to produce a wrong-string bug rather than a crash, so step 7 needs a test that forces
collection inside natives that build strings (`Array.prototype.join`, `JSON.stringify`,
template literals).

**VmState across processes (`value/wire.rs`).** State is JSON and must keep decoding states
stored by older builds. The plan keeps the wire form exactly: a `Val` string serializes as the
string's text (resolving its handle through a thread-local set during encoding, the pattern
`with_string_identities` already uses), atoms serialize as text, and decoding interns. The
conformance-only identity encodings (`g`, `j` string ages, slice parents) stay attached to
`JsString` in the table, not to the handle. `common::round_trip` and the snapshot suites hold
this; add a fixture of a state encoded before the change and decode it after.

**The conformance feature.** `Smi`, `SharedNumber`, `LocalNumber` and the string cells with
identity, age, externalization and forwarding all exist only under `conformance`. The tag space
in 3.1 reserves tags 5 and 6 for them, and the string table stores the conformance `JsString`
with its `Storage` metadata unchanged, so identity semantics (`same_backing`, `is_interned`)
still hold per cell. The conformance property-read cost (`is_shared_object` on every load)
needs its own fix, a flag on the object set when it becomes shared, or the gate cannot halve
that row in that configuration.

**Wasm.** `builtins/wasm/execute.rs` keeps its own `stack: Vec<JsValue>` and frames. Wasm values
are i32, i64, f32, f64, v128 and references; they never needed `JsValue`. Leave Wasm on
`JsValue` during this work and convert at the JS boundary as today; moving Wasm to its own
untagged value stack is a separate task.

**Shared code and atoms across threads.** Compiled `Code` is shared through the process cache
and its inline caches are atomics. Atoms must therefore be process-wide, not per VM, and the
table must be safe to read from many page threads. A thread-local table would make `Code::atoms`
wrong on another thread.

**Heap overshoot and bounds.** Strings counted in a table instead of by weak registration change
when string-unit limits trip. The `bounds.rs` and `string_budget` tests decide whether the new
accounting is at least as strict.

## 6. Expected gain per row (browser configuration)

| Row | Now | Main lever | Expected after steps 2 to 8 |
|:---|---:|:---|---:|
| arithmetic | 156 | 5 ops to 1 (step 4), loop ops 10 to 3, no drop | 50 to 70 |
| call | 286 | Copy values in args and frame, prelude, no `args` Vec copy per call | 130 to 160 |
| property read | 176 | `GetPropR`, Copy values | 70 to 90 |
| new | 989 | no `prototype` allocation, cached `prototype` load, cheaper alloc | 400 to 500 |
| indexed read | 182 | `GetElemRR`, Copy values | 70 to 90 |

These are estimates from the profile shares, not measurements. `new` is the row most at risk of
missing a full halving, because about 12% of it is inserting into the persistent object trie,
which C3 does not change. If the gate must hold for `new`, the trie insert path (`insert_shared`
copying a 64-slot leaf) needs its own look.

## 7. How to measure

- The pinned benchmark from step 1: best of three, one million iterations, rows in a function and
  at top level, reported as ns per iteration and ns over `x=a`. Run
  `cargo test --release --offline -p mars-v9 --test perf_ops -- --ignored --nocapture` and the
  same with `--features conformance`. A row passes when it is at most half its baseline in both.
- Profile any row that misses with `sample <pid> 3` on the test binary (`C3_ROW=<row>` selecting
  one row in a loop), aggregating self time per function.
- Correctness: every V9 test target in both configurations, plus the mjsunit and test262 runners,
  compared against the counts recorded before step 1.
