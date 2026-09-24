/**
 * Unit tests for the product channel's cards: the layout every card shares,
 * durations a person can read, and the rules that keep routine events (a
 * reconnect, a short session, an app updating itself) out of the channel.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { card, duration } from '../../../../src/modules/telemetry/cards.ts';

/** A person with an email, as a card names them. */
const ANA = { id: 'u-1', email: 'ana@example.com', label: 'ana@example.com' };

describe('product cards', () => {
  it('lays a card out as event, user, details and time between two rules', () => {
    const lines = card('playbook_saved', ANA, { steps: 3 })!.split('\n');
    assert.equal(lines[0], lines.at(-1));
    assert.match(lines[0], /^━+$/);
    assert.deepEqual(lines.slice(1, 5), ['📌 Event: playbook_saved', '👤 User: ana@example.com', '', '📋 Steps: 3']);
    assert.match(lines.at(-2)!, /^🕒 Time: \d{4}-\d\d-\d\dT/);
  });

  it('names a visitor with no label as an anonymous visitor', () => {
    const text = card(
      'download_served',
      { id: 'v', label: '' },
      {
        platform: 'mac',
        version: '1.0.121',
        file_type: 'installer',
        via: 'web',
      },
    );
    assert.match(text!, /👤 User: anonymous visitor/);
  });

  it('says how a sign-up was made', () => {
    assert.match(card('account_signed_up', ANA, { method: 'google' })!, /🔐 Method: google/);
  });

  it('posts a started browser only when it is the key’s first', () => {
    const props = { provider: 'oya', persona: false, via: 'mcp' as const };
    assert.equal(card('browser_started', ANA, { ...props, first: false }), null);
    assert.match(card('browser_started', ANA, { ...props, first: true })!, /🚀 First browser on this key/);
  });

  it('posts a session only once it lasted a minute, with its length', () => {
    assert.equal(card('browser_stopped', ANA, { provider: 'oya', seconds: 59 }), null);
    assert.match(card('browser_stopped', ANA, { provider: 'oya', seconds: 252 })!, /⏱️ Session: 4m 12s/);
  });

  it('posts a desktop only on its first connect', () => {
    const props = { platform: 'Win32', version: '1.0.121' };
    assert.equal(card('desktop_connected', ANA, { ...props, first: false }), null);
    assert.ok(card('desktop_connected', ANA, { ...props, first: true }));
  });

  it('posts a person’s installer download, never the app fetching its update', () => {
    const base = { platform: 'mac', version: '1.0.121' };
    assert.ok(card('download_served', ANA, { ...base, file_type: 'installer', via: 'web' }));
    assert.equal(card('download_served', ANA, { ...base, file_type: 'update', via: 'updater' }), null);
  });

  it('says how a replay ended', () => {
    const text = card('playbook_replayed', ANA, { outcome: 'handed_over', steps: 5, healed: false });
    assert.match(text!, /🙋 Handed to a person/);
  });

  it('posts nothing for an event with no card, such as an update check', () => {
    assert.equal(card('update_checked', ANA, { platform: 'mac', from_version: '1.0.120' }), null);
    assert.equal(card('mcp_tool_called', ANA, { tool: 'click' }), null);
  });

  it('writes a duration in seconds, minutes or hours', () => {
    assert.deepEqual([duration(42), duration(252), duration(7500), duration(-3)], ['42s', '4m 12s', '2h 5m', '0s']);
  });
});
