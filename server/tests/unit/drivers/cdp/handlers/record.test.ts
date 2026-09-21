/**
 * Unit tests for the record handler against a fake CDP connection: starting
 * captures the starting page, polling answers the whole buffer, stopping keeps
 * it, and a recording that cannot arm is not left claiming to run.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { fakeDriver } from '../../../support/cdp.ts';

/**
 * A driver on a page whose recorder connects: the page answers the channel's
 * readiness call on the binding it was given, as a real page would.
 */
function onPage(url = 'https://site.example/start') {
  const made = fakeDriver();
  let binding = '';
  made.conn.replies['Runtime.addBinding'] = (p: any) => void (binding = p.name);
  made.conn.evaluate = (e) => {
    const call = `window[${JSON.stringify(binding)}](`;
    if (binding && e.startsWith(call)) {
      const payload = JSON.parse(e.slice(call.length, -1));
      made.conn.emit('Runtime.bindingCalled', { name: binding, payload });
    }
    return e === 'location.href' ? url : undefined;
  };
  return made;
}

describe('record', () => {
  it('starts with the page it is on as the first step', async () => {
    const { driver } = onPage();
    const result = await driver.dispatch('record', { mode: 'start' });
    assert.equal(result.data.recording, true);
    assert.deepEqual(
      result.data.steps.map((s: any) => [s.action, s.url, s.start]),
      [['navigate', 'https://site.example/start', true]],
    );
    await driver.dispatch('record', { mode: 'stop' });
  });

  it('records no starting step on a page that is not http', async () => {
    const { driver } = onPage('about:blank');
    assert.deepEqual((await driver.dispatch('record', { mode: 'start' })).data.steps, []);
    await driver.dispatch('record', { mode: 'stop' });
  });

  it('answers the whole buffer on a poll, collecting new steps once each', async () => {
    const { driver } = onPage();
    await driver.dispatch('record', { mode: 'start' });
    driver.collectRecording({ steps: [{ id: 'a', action: 'click' }], secrets: ['PASSWORD'] });
    driver.collectRecording({
      steps: [
        { id: 'a', action: 'click' },
        { id: 'b', action: 'type' },
      ],
    });
    const result = await driver.dispatch('record', {});
    assert.deepEqual(
      result.data.steps.map((s: any) => s.action),
      ['navigate', 'click', 'type'],
    );
    assert.deepEqual(result.data.secrets, ['PASSWORD']);
    await driver.dispatch('record', { mode: 'stop' });
  });

  it('stops but keeps the buffer for the answer', async () => {
    const { driver } = onPage();
    await driver.dispatch('record', { mode: 'start' });
    const result = await driver.dispatch('record', { mode: 'stop' });
    assert.equal(result.data.recording, false);
    assert.equal(result.data.steps.length, 1);
    assert.equal(driver.recordChannel, null);
  });

  it('ignores steps while not recording', async () => {
    const { driver } = onPage();
    driver.collectRecording({ steps: [{ id: 'x' }] });
    assert.deepEqual((await driver.dispatch('record', {})).data, { recording: false, steps: [], secrets: [] });
  });

  it('is not left recording when the page cannot be armed', async () => {
    const { driver, conn } = onPage();
    conn.replies['Runtime.addBinding'] = new Error('target closed');
    await assert.rejects(driver.dispatch('record', { mode: 'start' }), /target closed/);
    assert.equal(driver.recording, false);
  });
});
