/**
 * Unit tests for the companion's narration: one short line per agent action,
 * naming the element from the last analysis and never what was typed.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { lineFor, namesFrom, nameOf, changesPage } = require('../../../../main/shell/narration.cjs');
const { NARRATION_NAME_MAX } = require('../../../../main/shell/constants.cjs');

describe('narration', () => {
  // The analysis names its own randomised attribute; the server aims actions with data-ac-id. Only the id matches.
  const names = namesFrom([
    { id: 1, selector: '[data-r4nd0m="1"]', text: 'Start  now' },
    { id: 2, selector: '[data-r4nd0m="2"]', label: 'Email', placeholder: 'you@example.com' },
  ]);

  it('names the element an action is on, by the label, text or placeholder the analysis read', () => {
    assert.equal(lineFor('click', { selector: '[data-ac-id="1"]' }, names), 'Clicking “Start now”');
    assert.equal(lineFor('type', { selector: '[data-ac-id="2"]', text: 'x' }, names), 'Typing into “Email”');
  });

  it('never repeats what was typed or chosen', () => {
    const typed = lineFor('type', { selector: '[data-ac-id="2"]', text: 'hunter2' }, names);
    const chosen = lineFor('select', { selector: '[data-ac-id="2"]', value: '1990-01-01' }, names);
    assert.doesNotMatch(`${typed} ${chosen}`, /hunter2|1990/);
  });

  it('says the plain verb when the element has no name it knows', () => {
    assert.equal(lineFor('click', { selector: '[data-ac-id="9"]' }, names), 'Clicking');
  });

  it('marks the actions that change the page, and not those that only look at it', () => {
    assert.ok(changesPage('click') && changesPage('type') && changesPage('navigate'));
    assert.ok(!changesPage('screenshot') && !changesPage('scroll') && !changesPage('hover'));
  });

  it('opens a site by its host name, and says nothing for an action it does not narrate', () => {
    assert.equal(
      lineFor('navigate', { url: 'https://www.jumpermedia.co/signup?a=1' }, names),
      'Opening jumpermedia.co',
    );
    assert.equal(lineFor('evaluate_raw', {}, names), '');
    assert.equal(lineFor('constructor', {}, names), '');
  });

  it('shortens a long name with an ellipsis', () => {
    const name = nameOf({ text: 'a'.repeat(NARRATION_NAME_MAX * 2) });
    assert.equal(name.length, NARRATION_NAME_MAX);
    assert.ok(name.endsWith('…'));
  });
});
