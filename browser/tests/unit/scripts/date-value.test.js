/**
 * Unit tests for scripts/date-value.cjs: the value a native date or time input
 * takes from the text an agent meant to type.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { isDateInput, dateInputValue, unreadableDate } = require('../../../scripts/date-value.cjs');

describe('isDateInput', () => {
  it('knows the native date and time input types, in any case', () => {
    for (const type of ['date', 'datetime-local', 'month', 'week', 'time', 'DATE']) assert.ok(isDateInput(type), type);
  });

  it('leaves text-like inputs to typing', () => {
    for (const type of ['text', 'email', 'number', 'search', '', undefined]) assert.equal(isDateInput(type), false);
  });
});

describe('dateInputValue: date', () => {
  it('keeps a year-first date, padding its month and day', () => {
    assert.equal(dateInputValue('date', '2024-01-15'), '2024-01-15');
    assert.equal(dateInputValue('date', '2024-1-5'), '2024-01-05');
  });

  it('reads a numbers-only date month first, as US forms write it', () => {
    assert.equal(dateInputValue('date', '01/05/2024'), '2024-01-05');
    assert.equal(dateInputValue('date', '1-5-2024'), '2024-01-05');
  });

  it('reads it day first when the first number cannot be a month', () => {
    assert.equal(dateInputValue('date', '15.01.2024'), '2024-01-15');
  });

  it('places a two-digit year in this century', () => {
    assert.equal(dateInputValue('date', '03/04/99'), '2099-03-04');
  });

  it('reads a date written in words', () => {
    assert.equal(dateInputValue('date', 'January 15, 2024'), '2024-01-15');
    assert.equal(dateInputValue('date', '15 Jan 2024'), '2024-01-15');
  });

  it('ignores surrounding spaces and a trailing time', () => {
    assert.equal(dateInputValue('date', '  2024-01-15T09:30  '), '2024-01-15');
  });

  it('refuses a day that does not exist', () => {
    assert.equal(dateInputValue('date', '2023-02-29'), null);
    assert.equal(dateInputValue('date', '04/31/2024'), null);
    assert.equal(dateInputValue('date', '2024-13-01'), null);
  });

  it('refuses text that is not a date', () => {
    for (const text of ['tomorrow', '', '2024', 'abc']) assert.equal(dateInputValue('date', text), null, text);
  });
});

describe('dateInputValue: other types', () => {
  it('gives a month input the year and month', () => {
    assert.equal(dateInputValue('month', '2024-03'), '2024-03');
    assert.equal(dateInputValue('month', '03/15/2024'), '2024-03');
  });

  it('gives a time input a 24-hour time', () => {
    assert.equal(dateInputValue('time', '15:30'), '15:30');
    assert.equal(dateInputValue('time', '3:05 PM'), '15:05');
    assert.equal(dateInputValue('time', '12:10 a.m.'), '00:10');
    assert.equal(dateInputValue('time', '25:00'), null);
  });

  it('gives a datetime-local input a date and time, midnight when no time is given', () => {
    assert.equal(dateInputValue('datetime-local', '2024-01-15 3:30 pm'), '2024-01-15T15:30');
    assert.equal(dateInputValue('datetime-local', '01/15/2024'), '2024-01-15T00:00');
  });

  it('takes a week only as an ISO week', () => {
    assert.equal(dateInputValue('week', '2024-W03'), '2024-W03');
    assert.equal(dateInputValue('week', '2024-01-15'), null);
  });

  it('has no value for a type that is not a date input', () => {
    assert.equal(dateInputValue('text', '2024-01-15'), null);
  });
});

describe('unreadableDate', () => {
  it('tells the agent the format the input takes', () => {
    assert.equal(
      unreadableDate('date', 'next Friday'),
      'Could not read "next Friday" as a date value. Type it as YYYY-MM-DD.',
    );
  });
});
