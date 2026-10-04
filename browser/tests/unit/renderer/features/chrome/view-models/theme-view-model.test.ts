/**
 * Unit tests for the shell's theme: system follows the OS, the saved
 * preferences and platform restore at start, a pick is saved, and every
 * change turns transitions off for two frames.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  ThemeViewModel,
  appliedTheme,
  themeOf,
} from '../../../../../../src/renderer/features/chrome/view-models/theme-view-model.ts';
import { fakeBridge, manualFrames } from '../../../support/bridge.ts';

/** Lets the fake bridge's promises settle. */
const settle = () => new Promise<void>((resolve) => setImmediate(resolve));

/** A theme over a fake bridge answering `preferences`. */
function theme(preferences: unknown = {}, systemDark = false) {
  const fake = fakeBridge({ getUiPreferences: preferences });
  const frames = manualFrames();
  return { fake, frames, vm: new ThemeViewModel({ bridge: fake.bridge, frames, systemDark }) };
}

describe('ThemeViewModel', () => {
  it('starts on system, in the appearance of the first paint', () => {
    const { vm } = theme({}, true);
    assert.equal(vm.state.theme, 'system');
    assert.equal(appliedTheme(vm.state), 'dark');
  });

  it('follows the OS while on system', () => {
    const { fake, vm } = theme();
    fake.emit('onShellAppearance', true);
    assert.equal(appliedTheme(vm.state), 'dark');
  });

  it('keeps a picked theme whatever the OS does', () => {
    const { fake, vm } = theme();
    vm.choose('light');
    fake.emit('onShellAppearance', true);
    assert.equal(appliedTheme(vm.state), 'light');
  });

  it('saves a picked theme', () => {
    const { fake, vm } = theme();
    vm.choose('dark');
    assert.deepEqual(fake.called('saveUiPreferences'), [[{ theme: 'dark' }]]);
  });

  it('shows a theme without saving it', () => {
    const { fake, vm } = theme();
    vm.show('dark');
    assert.equal(appliedTheme(vm.state), 'dark');
    assert.equal(fake.called('saveUiPreferences').length, 0);
  });

  it('restores the saved theme, the OS appearance and the platform', async () => {
    const { vm } = theme({ theme: 'dark', systemDark: false, platform: 'darwin' }, true);
    await settle();
    assert.deepEqual([vm.state.theme, vm.state.systemDark, vm.state.platform], ['dark', false, 'darwin']);
  });

  it('falls back to system for an unknown saved theme, or none', async () => {
    const unknown = theme({ theme: 'sepia' });
    const none = theme(null);
    await settle();
    assert.equal(unknown.vm.state.theme, 'system');
    assert.equal(none.vm.state.theme, 'system');
    assert.equal(themeOf('light'), 'light');
  });

  it('turns transitions off for two frames on each change', () => {
    const { frames, vm } = theme();
    frames.run();
    frames.run();
    assert.equal(vm.state.switching, false);
    vm.choose('dark');
    assert.equal(vm.state.switching, true);
    frames.run();
    assert.equal(vm.state.switching, true, 'still off after one frame');
    frames.run();
    assert.equal(vm.state.switching, false);
  });

  it('stops following the OS and its frames on dispose', () => {
    const { fake, frames, vm } = theme();
    vm.dispose();
    assert.equal(fake.listeners('onShellAppearance'), 0);
    assert.equal(frames.waiting, 0);
  });
});
