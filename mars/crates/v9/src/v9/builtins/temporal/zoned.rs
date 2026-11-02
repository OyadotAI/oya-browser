/** Temporal.ZonedDateTime: an exact instant read in a named zone or fixed offset, with the offset in force at that instant. */
use super::duration::UNIT_NS;
use super::instant::{new_instant, parts, time_zone};
use super::iso::{self, NS_MAX};
use super::plain::format_date;
use super::*;

/** `new Temporal.ZonedDateTime(epochNanoseconds, timeZone)`: the zone's offset is read at the given instant. */
fn ctor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Temporal.ZonedDateTime requires 'new'"));
    };
    let ns = super::super::bigint::to_bigint_checked(vm, &call.arg(0))?;
    if ns.abs() > NS_MAX {
        return Err(vm.range_error("instant outside the supported range"));
    }
    let (id, offset) = time_zone(vm, &call.arg(1), ns)?;
    make(
        vm,
        Some(&target),
        T {
            kind: Kind::Zoned,
            v: vec![ns, offset],
            tz: id,
        },
    )
}

/** The slots of `this`, checked to be a ZonedDateTime. */
fn this_zoned(vm: &mut Vm, call: &Call) -> Res<T> {
    this_t(vm, call, Kind::Zoned)
}

/** A wall-clock date and time: the ISO date and the six clock fields. */
type WallFields = ((i64, i64, i64), [i64; 6]);

/** The wall-clock date and time: the exact instant shifted by the offset in force, so a getter never reads UTC. */
fn wall(t: &T) -> WallFields {
    parts(t.v[0] + t.v[1])
}

/** One wall-clock field of `this`, picked out of the wall date and time. */
fn wall_field(vm: &mut Vm, call: &Call, pick: fn(&WallFields) -> i64) -> Res<JsValue> {
    let t = this_zoned(vm, call)?;
    Ok(JsValue::Number(pick(&wall(&t)) as f64))
}

/** Generate a getter for each wall-clock field, and the table that installs them. */
macro_rules! wall_getters {
    ($($f:ident => $pick:expr),* $(,)?) => {
        $(fn $f(vm: &mut Vm, call: Call) -> Res<JsValue> { wall_field(vm, &call, $pick) })*
    };
}

wall_getters!(
    z_year => |w| w.0.0,
    z_month => |w| w.0.1,
    z_day => |w| w.0.2,
    z_hour => |w| w.1[0],
    z_minute => |w| w.1[1],
    z_second => |w| w.1[2],
    z_millisecond => |w| w.1[3],
    z_microsecond => |w| w.1[4],
    z_nanosecond => |w| w.1[5],
    z_day_of_week => |w| iso::day_of_week(w.0.0, w.0.1, w.0.2),
);

/** ZonedDateTime.prototype.offset: the signed offset in force, as "+HH:MM". */
fn offset(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_zoned(vm, &call)?;
    js(&iso::format_offset(t.v[1]))
}

/** ZonedDateTime.prototype.timeZoneId: the canonical identifier the value was created with. */
fn time_zone_id(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_zoned(vm, &call)?;
    js(&t.tz)
}

/** ZonedDateTime.prototype.epochNanoseconds: the exact instant as a BigInt. */
fn epoch_ns(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_zoned(vm, &call)?;
    Ok(JsValue::BigInt(t.v[0]))
}

/** ZonedDateTime.prototype.epochMilliseconds: the instant floored to whole milliseconds. */
fn epoch_ms(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_zoned(vm, &call)?;
    Ok(JsValue::Number(t.v[0].div_euclid(UNIT_NS[7]) as f64))
}

/** ZonedDateTime.prototype.toInstant: the same exact instant, without its zone. */
fn to_instant(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_zoned(vm, &call)?;
    new_instant(vm, None, t.v[0])
}

/** The default ZonedDateTime text: wall date and time, offset, then the zone in brackets. */
fn zoned_text(t: &T) -> String {
    let (date, time) = wall(t);
    format!(
        "{}T{}{}[{}]",
        format_date(date),
        iso::format_time(&time, None),
        iso::format_offset(t.v[1]),
        t.tz
    )
}

/** ZonedDateTime.prototype.toString with default options: the same text toJSON gives. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let t = this_zoned(vm, &call)?;
    js(&zoned_text(&t))
}

/** ZonedDateTime.prototype.toJSON: the default text. */
fn to_json(vm: &mut Vm, call: Call) -> Res<JsValue> {
    to_string(vm, call)
}

/** Install Temporal.ZonedDateTime. */
pub(super) fn install(b: &mut Builder, ns: ObjectId, protos: &mut [ObjectId; 8]) {
    protos[Kind::Zoned as usize] = class(
        b,
        ns,
        Kind::Zoned,
        ctor,
        2,
        &[],
        &[
            ("toInstant", 0, to_instant),
            ("toString", 0, to_string),
            ("toJSON", 0, to_json),
        ],
        &[
            ("offset", offset),
            ("timeZoneId", time_zone_id),
            ("epochNanoseconds", epoch_ns),
            ("epochMilliseconds", epoch_ms),
            ("year", z_year),
            ("month", z_month),
            ("day", z_day),
            ("hour", z_hour),
            ("minute", z_minute),
            ("second", z_second),
            ("millisecond", z_millisecond),
            ("microsecond", z_microsecond),
            ("nanosecond", z_nanosecond),
            ("dayOfWeek", z_day_of_week),
        ],
    );
}
