/**
 * Unit tests for how a command is described for the activity log: only a snapshot the server
 * marks with its own QUIET symbol stays out of the log, and nothing a caller sends can hide a
 * command from the person watching the browser.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { QUIET, describeCall } from '../../../../../src/modules/browsers/connection/reporter.ts';

describe('describeCall', () => {
  it('keeps the server’s own marked snapshot out of the activity log', () => {
    assert.equal(describeCall('b1', 'screenshot', { [QUIET]: true }, 1000).visible, false);
  });

  it('never lets a caller hide a command with a quiet field, which any JSON body or tool argument can carry', () => {
    const call = describeCall('b1', 'click', { selector: '#buy', quiet: true }, 1000);
    assert.equal(call.visible, true);
    assert.equal(describeCall('b1', 'screenshot', { quiet: true }, 1000).visible, true);
  });

  it('never hides anything but a snapshot, even with the server’s mark', () => {
    for (const action of ['click', 'type', 'navigate', 'run_script'])
      assert.equal(describeCall('b1', action, { [QUIET]: true }, 1000).visible, true, action);
  });

  it('sends the browser no trace of the mark: it does not survive JSON', () => {
    const call = describeCall('b1', 'screenshot', { format: 'jpeg', [QUIET]: true }, 1000);
    assert.deepEqual(JSON.parse(JSON.stringify(call.params)), { format: 'jpeg' });
  });
});
