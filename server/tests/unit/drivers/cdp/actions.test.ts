/**
 * Unit tests for the CDP action vocabulary: the advertised capabilities and
 * how the Oya client's spellings map onto the driver's own.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { CDP_CAPABILITIES, normalise } from '../../../../src/drivers/cdp/actions.ts';

describe('normalise', () => {
  it('passes an inherited property name through as itself, not as an Object prototype member', () => {
    for (const name of ['constructor', 'toString', '__proto__']) assert.equal(normalise(name, {}).action, name);
  });

  it('maps the Oya client’s underscored names onto the driver’s', () => {
    for (const [from, to] of [
      ['click_coordinates', 'click-coords'],
      ['press_key', 'press-key'],
      ['list_tabs', 'list-tabs'],
      ['open_tab', 'new-tab'],
      ['new_tab', 'new-tab'],
      ['switch_tab', 'switch-tab'],
      ['close_tab', 'close-tab'],
      ['read_elements', 'read_page'],
    ]) {
      assert.equal(normalise(from, {}).action, to, from);
    }
  });

  it('leaves the driver’s own names alone', () => {
    assert.deepEqual(normalise('click', { element_id: 1 }), { action: 'click', params: { element_id: 1 } });
  });

  it('moves the scroll direction into the name, down by default', () => {
    assert.equal(normalise('scroll', { direction: 'up' }).action, 'scroll-up');
    assert.equal(normalise('scroll').action, 'scroll-down');
  });

  it('names a tab `id` whichever way the caller spelled it', () => {
    assert.deepEqual(normalise('switch_tab', { tab_id: 't1' }).params, { tab_id: 't1', id: 't1' });
    assert.deepEqual(normalise('close-tab', { id: 't2' }).params, { id: 't2' });
  });
});

describe('CDP_CAPABILITIES', () => {
  it('lists both spellings, so no caller thinks a supported action is missing', () => {
    for (const action of ['press-key', 'press_key', 'list-tabs', 'list_tabs', 'switch_tab', 'handle_dialog']) {
      assert.ok(CDP_CAPABILITIES.has(action), action);
    }
  });
});
