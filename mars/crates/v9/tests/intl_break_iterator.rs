/**
 * `Intl.v8BreakIterator`, V8's non-standard break iterator: present only in conformance builds
 * (the `conformance` feature), so the upstream mjsunit tests that construct it can run. These tests
 * pin its contract: the cursor walk, UTF-16 positions, break types, the getter-closure shape, and
 * the failure paths. A normal build must not expose it at all.
 */
use crate::common;

use common::eval;

/** A normal build has no v8BreakIterator: the conformance surface stays out of the browser. */
#[cfg(not(feature = "conformance"))]
#[test]
fn break_iterator_is_absent_without_conformance() {
    assert_eq!(
        eval("const out = typeof Intl.v8BreakIterator;", "out"),
        "undefined"
    );
}

/** The constructor resolves en-US and reads word cuts when the page names no type. */
#[cfg(feature = "conformance")]
#[test]
fn constructor_defaults_to_word_cuts() {
    let out = eval(
        "const o = new Intl.v8BreakIterator().resolvedOptions(); const out = o.locale + '|' + o.type;",
        "out",
    );
    assert_eq!(out, "en-US|word");
}

/** Word cuts walk runs of letters and digits and each other character, in order, then run off the end. */
#[cfg(feature = "conformance")]
#[test]
fn steps_walk_word_boundaries_then_report_done() {
    let out = eval(
        "const it = new Intl.v8BreakIterator(); it.adoptText('Hi, there!');
         const seen = [it.first()]; for (let at = it.next(); at !== -1; at = it.next()) seen.push(at);
         const out = seen.join() + '|' + it.current() + '|' + it.next();",
        "out",
    );
    assert_eq!(out, "0,2,3,4,9,10|-1|-1");
}

/** Positions are UTF-16 code units, so an astral character before a boundary counts twice. */
#[cfg(feature = "conformance")]
#[test]
fn positions_count_utf16_units() {
    let out = eval(
        "const it = new Intl.v8BreakIterator(); it.adoptText('\u{1F600} a');
         const out = [it.first(), it.next(), it.next(), it.next()].join();",
        "out",
    );
    assert_eq!(out, "0,2,3,4");
}

/** breakType names the segment the cursor last passed: letters, none, number, kana and ideo. */
#[cfg(feature = "conformance")]
#[test]
fn break_type_names_each_word_script() {
    let out = eval(
        "const it = new Intl.v8BreakIterator(); it.adoptText('abc 123 \u{3072}\u{3089} \u{4E2D}');
         const types = [it.breakType()]; while (it.next() !== -1) types.push(it.breakType());
         const out = types.join();",
        "out",
    );
    assert_eq!(out, "none,letter,none,number,none,kana,none,ideo");
}

/** breakType is unknown once the iterator has run off the end. */
#[cfg(feature = "conformance")]
#[test]
fn break_type_is_unknown_after_done() {
    let out = eval(
        "const it = new Intl.v8BreakIterator(); it.adoptText('ab'); it.next(); it.next();
         const out = it.breakType();",
        "out",
    );
    assert_eq!(out, "unknown");
}

/** first rewinds to the first boundary from any later position. */
#[cfg(feature = "conformance")]
#[test]
fn first_rewinds_to_the_start() {
    let out = eval(
        "const it = new Intl.v8BreakIterator(); it.adoptText('a b'); it.next(); it.next();
         const out = it.first() + '|' + it.current();",
        "out",
    );
    assert_eq!(out, "0|0");
}

/** Empty text has one boundary, so the first step is done. */
#[cfg(feature = "conformance")]
#[test]
fn empty_text_is_done_at_once() {
    let out = eval(
        "const it = new Intl.v8BreakIterator(); it.adoptText(''); const out = it.first() + '|' + it.next();",
        "out",
    );
    assert_eq!(out, "0|-1");
}

/** Sentence cuts fall after each terminator, and the tail is its own sentence. */
#[cfg(feature = "conformance")]
#[test]
fn sentence_type_cuts_after_terminators() {
    let out = eval(
        "const it = new Intl.v8BreakIterator(undefined, { type: 'sentence' });
         it.adoptText('One. Two! Three'); const out = [it.first(), it.next(), it.next(), it.next(), it.next()].join();",
        "out",
    );
    assert_eq!(out, "0,4,9,15,-1");
}

/** Character cuts put every scalar value in its own segment. */
#[cfg(feature = "conformance")]
#[test]
fn character_type_cuts_each_scalar() {
    let out = eval(
        "const it = new Intl.v8BreakIterator(undefined, { type: 'character' });
         it.adoptText('a\u{00F1}'); const out = [it.first(), it.next(), it.next()].join();",
        "out",
    );
    assert_eq!(out, "0,1,2");
}

/** A type the iterator does not know is a RangeError, not a silent default. */
#[cfg(feature = "conformance")]
#[test]
fn unknown_type_is_a_range_error() {
    let out = eval(
        "let out; try { new Intl.v8BreakIterator(undefined, { type: 'emoji' }); out = 'none'; } catch (e) { out = e.name; }",
        "out",
    );
    assert_eq!(out, "RangeError");
}

/** A step read from the prototype with another receiver throws, rather than reading foreign state. */
#[cfg(feature = "conformance")]
#[test]
fn step_getter_rejects_a_foreign_receiver() {
    let out = eval(
        "let out; try { Object.getOwnPropertyDescriptor(Intl.v8BreakIterator.prototype, 'next').get.call({}); out = 'none'; } catch (e) { out = e.name; }",
        "out",
    );
    assert_eq!(out, "TypeError");
}

/** A step taken off the object keeps its receiver, as V8's bound steps do. */
#[cfg(feature = "conformance")]
#[test]
fn detached_step_keeps_its_iterator() {
    let out = eval(
        "const it = new Intl.v8BreakIterator(); it.adoptText('a b'); const next = it.next;
         const out = [next(), next()].join();",
        "out",
    );
    assert_eq!(out, "1,2");
}

/** The getters are accessors named `get <step>` with no parameters, as mjsunit checks them. */
#[cfg(feature = "conformance")]
#[test]
fn step_getters_are_named_accessors() {
    let out = eval(
        "const d = Object.getOwnPropertyDescriptor(Intl.v8BreakIterator.prototype, 'breakType');
         const out = d.get.name + '|' + d.get.length + '|' + d.enumerable + '|' + d.configurable;",
        "out",
    );
    assert_eq!(out, "get breakType|0|false|true");
}

/** The constructor's prototype is read-only and supportedLocalesOf echoes the requested tags. */
#[cfg(feature = "conformance")]
#[test]
fn constructor_prototype_is_read_only_and_locales_echo() {
    let out = eval(
        "const d = Object.getOwnPropertyDescriptor(Intl.v8BreakIterator, 'prototype');
         const out = [d.writable, d.configurable, Intl.v8BreakIterator.supportedLocalesOf(['de']).join()].join('|');",
        "out",
    );
    assert_eq!(out, "false|false|de");
}
