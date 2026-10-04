/** Unit tests for src/main/recording/tab-names.ts (RecordingTabNames): stable names, and the first free tab-N. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { RecordingTabNames } from '../../../../src/main/recording/tab-names.ts';

describe('RecordingTabNames', () => {
  it('gives each tab one name for good', () => {
    const names = new RecordingTabNames(() => []);
    assert.equal(names.recordingTab(7), 'tab-1');
    assert.equal(names.recordingTab(8), 'tab-2');
    assert.equal(names.recordingTab(7), 'tab-1');
  });

  it('skips names already used by recorded steps or set by hand', () => {
    const names = new RecordingTabNames(() => [{ tab: 'tab-1' }]);
    names.set(1, 'tab-2');
    assert.equal(names.recordingTab(3), 'tab-3');
    assert.equal(names.size, 2);
    names.clear();
    assert.equal(names.size, 0);
  });
});
