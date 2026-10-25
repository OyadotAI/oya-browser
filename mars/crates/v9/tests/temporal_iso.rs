/** Temporal over the ISO 8601 calendar, UTC and fixed offsets: one test per ported rule. */
mod common;
use common::eval;

/** PlainDate reads ISO strings, derives week and day fields, and adds with constrain. */
#[test]
fn plain_date_parses_derives_and_adds() {
    let source = "const d = Temporal.PlainDate.from('2021-01-31');
        const out = [d.toString(), d.dayOfWeek, d.weekOfYear, d.yearOfWeek,
            d.add({ months: 1 }).toString(), d.toString({ calendarName: 'always' })].join();";
    assert_eq!(
        eval(source, "out"),
        "2021-01-31,7,4,2021,2021-02-28,2021-01-31[u-ca=iso8601]"
    );
}

/** Bags are read alphabetically and reject a month that disagrees with monthCode. */
#[test]
fn property_bags_read_in_order_and_check_month_code() {
    let source = "const seen = [];
        const bag = new Proxy({ year: 2020, month: 2, day: 30 }, { get(t, k) { seen.push(k); return t[k]; } });
        const d = Temporal.PlainDate.from(bag);
        let threw = false;
        try { Temporal.PlainDate.from({ year: 2020, month: 1, monthCode: 'M02', day: 1 }); } catch (e) { threw = e instanceof RangeError; }
        const out = [d.toString(), seen.join('|'), threw].join();";
    assert_eq!(
        eval(source, "out"),
        "2020-02-29,calendar|day|month|monthCode|year,true"
    );
}

/** PlainTime rounds, wraps past midnight, and formats to a requested precision. */
#[test]
fn plain_time_rounds_and_formats() {
    let source = "const t = new Temporal.PlainTime(23, 59, 59, 999);
        const out = [t.round('second').toString(), t.add({ hours: 2 }).toString(),
            t.toString({ fractionalSecondDigits: 1 }), t.toString({ smallestUnit: 'minute' })].join();";
    assert_eq!(
        eval(source, "out"),
        "00:00:00,01:59:59.999,23:59:59.9,23:59"
    );
}

/** PlainDateTime differences borrow a day when the clock runs backwards. */
#[test]
fn plain_date_time_until_borrows_a_day() {
    let source = "const a = Temporal.PlainDateTime.from('2020-01-01T12:00');
        const b = Temporal.PlainDateTime.from('2020-03-02T06:00');
        const out = [a.until(b).toString(), a.until(b, { largestUnit: 'month' }).toString(),
            a.since(b, { smallestUnit: 'day' }).toString()].join();";
    assert_eq!(eval(source, "out"), "P60DT18H,P2MT18H,-P60D");
}

/** Durations parse fractional hours, balance on add, and keep one sign. */
#[test]
fn durations_parse_balance_and_validate() {
    let source = "const d = Temporal.Duration.from('PT1.5H');
        let threw = false;
        try { new Temporal.Duration(1, -1); } catch (e) { threw = e instanceof RangeError; }
        const out = [d.minutes, d.add({ minutes: 45 }).toString(), d.negated().sign, threw,
            Temporal.Duration.from({ milliseconds: 1500 }).toString()].join();";
    assert_eq!(eval(source, "out"), "30,PT2H15M,-1,true,PT1.5S");
}

/** Instants read offsets, print UTC or a fixed offset, and refuse calendar units. */
#[test]
fn instants_use_utc_and_fixed_offsets() {
    let source = "const i = Temporal.Instant.from('2020-01-01T00:00+01:00');
        let threw = false;
        try { i.add({ days: 1 }); } catch (e) { threw = e instanceof RangeError; }
        const out = [i.toString(), i.toString({ timeZone: '-02:30' }), i.epochMilliseconds,
            typeof i.epochNanoseconds, threw].join();";
    assert_eq!(
        eval(source, "out"),
        "2019-12-31T23:00:00Z,2019-12-31T20:30:00-02:30,1577833200000,bigint,true"
    );
}

/** Year-months and month-days keep their reference fields and print without them. */
#[test]
fn year_month_and_month_day_reference_fields() {
    let source = "const ym = Temporal.PlainYearMonth.from('2019-12');
        const md = Temporal.PlainMonthDay.from({ monthCode: 'M02', day: 29 });
        const out = [ym.add({ months: 1 }).toString(), ym.daysInMonth, md.toString(),
            md.toPlainDate({ year: 2021 }).toString(), md.toString({ calendarName: 'always' })].join();";
    assert_eq!(
        eval(source, "out"),
        "2020-01,31,02-29,2021-02-28,1972-02-29[u-ca=iso8601]"
    );
}

/** Constructors need `new`, and methods check their receiver's brand. */
#[test]
fn constructors_and_brands() {
    let source = "const errs = [];
        try { Temporal.PlainDate(2020, 1, 1); } catch (e) { errs.push(e.constructor.name); }
        try { Temporal.PlainTime.prototype.toString.call(Temporal.PlainDate.from('2020-01-01')); } catch (e) { errs.push(e.constructor.name); }
        try { Temporal.PlainDate.from('2020-01-01') < 1; } catch (e) { errs.push(e.constructor.name); }
        const out = errs.join() + ',' + Object.prototype.toString.call(Temporal.Now);";
    assert_eq!(
        eval(source, "out"),
        "TypeError,TypeError,TypeError,[object Temporal.Now]"
    );
}
