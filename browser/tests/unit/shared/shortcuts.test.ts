/** Bindings and their reference labels must agree on every supported platform. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { SHORTCUTS, resolveShortcut, commandShortcut, shortcutLabel } from '../../../src/shared/shortcuts.ts';

/** Synthesizes an actual modifier combination from the documented binding. */
function inputFor(row, mac) {
  const mods = row[3];
  return {
    type: 'keyDown',
    key: row[4],
    code: row[5] || '',
    isAutoRepeat: false,
    meta: mods.startsWith('primary') && mac,
    control: mods.startsWith('control') || (mods.startsWith('primary') && !mac),
    shift: mods.endsWith('shift'),
    alt: mods.endsWith('alt'),
  };
}
describe('shared keyboard registry', () => {
  for (const platform of ['darwin', 'MacIntel', 'Win32', 'Linux']) {
    it(`dispatches every documented binding on ${platform}`, () => {
      for (const row of SHORTCUTS) {
        const input = inputFor(row, /darwin|Mac/.test(platform));
        assert.equal(resolveShortcut(input, platform), row[0], shortcutLabel(row, platform));
      }
    });
  }
  it('does not intercept typing, repeats, key-up or extra modifiers', () => {
    const input = inputFor(SHORTCUTS[0], true);
    for (const change of [
      { meta: false },
      { shift: true },
      { control: true },
      { alt: true },
      { isAutoRepeat: true },
      { type: 'keyUp' },
    ])
      assert.equal(resolveShortcut({ ...input, ...change }, 'darwin'), undefined);
  });
  it('uses physical keys for option chords on non-US layouts', () => {
    const row = SHORTCUTS.find(([id]) => id === 'record');
    assert.equal(resolveShortcut({ ...inputFor(row, true), key: '®' }, 'darwin'), 'record');
  });
  it('labels platform modifiers and unknown commands honestly', () => {
    assert.equal(commandShortcut('record', 'MacIntel'), '⌘ ⌥ R');
    assert.equal(commandShortcut('record', 'Win32'), 'Ctrl Alt R');
    assert.equal(commandShortcut('shortcuts', 'Linux'), 'Ctrl /');
    assert.equal(commandShortcut('missing', 'Linux'), '');
  });
});
