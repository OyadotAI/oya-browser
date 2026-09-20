/**
 * Unit tests for the element index appended to an analysis: one TOON table of
 * every form field with its label, value, format and state, one of the other
 * visible elements, and a capped list of off-screen ones.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { elementIndex, pageGuide } from '../../../../src/modules/agent/element-index.ts';
import { MAX_OFFSCREEN_LISTED } from '../../../../src/modules/agent/constants.ts';

/** The index's lines, trimmed. */
const lines = (index: string) => index.split('\n').map((l) => l.trim());

describe('elementIndex', () => {
  it('counts every element and the visible ones', () => {
    const index = elementIndex(
      [
        { id: 1, type: 'button', visible: true },
        { id: 2, type: 'link' },
      ],
      false,
    );
    assert.match(index, /## Element Index \(2 total, 1 visible\)/);
  });

  it('puts each field in one table: its kind, label, value, expected format and state', () => {
    const from = {
      id: 69,
      type: 'input',
      inputType: 'text',
      text: 'From Date',
      value: '02/32/026_',
      placeholder: '__/__/____',
      required: true,
      invalid: true,
      error: 'This field is invalid.',
      visible: true,
    };
    const index = lines(elementIndex([from], false));
    assert.ok(index.includes('fields[1]{id,type,label,value,hint,state}:'));
    assert.ok(index.includes('69,text,From Date,02/32/026_,__/__/____,"required invalid: This field is invalid."'));
  });

  it('gives a select its options as the hint', () => {
    const select = { id: 5, type: 'select', text: 'Review Type', value: 'x', options: 'A | B', visible: true };
    assert.ok(lines(elementIndex([select], false)).includes('5,select,Review Type,x,A | B,""'));
  });

  it('shows a checkbox by its state, not its value', () => {
    const box = { id: 2, type: 'checkbox', text: 'Agree', value: 'on', checked: true, disabled: true, visible: true };
    assert.ok(lines(elementIndex([box], false)).includes('2,checkbox,Agree,"","",checked disabled'));
  });

  it('keeps off-screen fields in the table, after the visible ones and marked off-screen', () => {
    const index = lines(
      elementIndex(
        [
          { id: 1, type: 'input', text: 'Later', visible: false },
          { id: 2, type: 'input', text: 'Now', visible: true },
        ],
        false,
      ),
    );
    const rows = index.filter((l) => /^\d,text,/.test(l));
    assert.deepEqual(rows, ['2,text,Now,"","",""', '1,text,Later,"","",off-screen']);
  });

  it('leaves out a hint that only repeats the label', () => {
    const field = { id: 3, type: 'input', text: 'Search', placeholder: 'Search', visible: true };
    assert.ok(lines(elementIndex([field], false)).includes('3,text,Search,"","",""'));
  });

  it('lists the other visible elements with their links, marking disabled ones', () => {
    const index = lines(
      elementIndex(
        [
          { id: 7, type: 'link', text: 'Help', href: 'https://a.test/help', visible: true },
          { id: 8, type: 'button', text: 'Next', disabled: true, visible: true },
        ],
        false,
      ),
    );
    assert.ok(index.includes('visible[2]{id,type,label,link}:'));
    assert.ok(index.includes('7,link,Help,"https://a.test/help"'));
    assert.ok(index.includes('8,button,Next (disabled),""'));
  });

  it('lists off-screen elements up to the cap and counts the rest', () => {
    const offscreen = Array.from({ length: MAX_OFFSCREEN_LISTED + 5 }, (_, i) => ({
      id: i,
      type: 'link',
      text: `L${i}`,
    }));
    const index = elementIndex(offscreen, false);
    assert.match(index, new RegExp(`offscreen\\[${MAX_OFFSCREEN_LISTED}\\]\\{id,type,label\\}:\\n {2}0,link,L0`));
    assert.doesNotMatch(index, new RegExp(`L${MAX_OFFSCREEN_LISTED}\\b`));
    assert.match(index, / {2}\.\.\. and 5 more off-screen elements/);
    assert.doesNotMatch(elementIndex([{ id: 1, type: 'link' }], false), /more off-screen/);
  });

  it('says when the page content was truncated', () => {
    assert.match(elementIndex([], true), /⚠ Page content was truncated/);
    assert.doesNotMatch(elementIndex([], false), /truncated/);
  });
});

describe('pageGuide', () => {
  it('explains every format when the server pins none, since each browser picks its own', () => {
    const guide = pageGuide();
    for (const format of ['as markdown', 'as TOON', 'as JSONL']) assert.ok(guide.includes(format), format);
  });
});
