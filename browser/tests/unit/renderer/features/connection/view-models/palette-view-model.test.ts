/**
 * Unit tests for the command palette: the commands and their shortcuts per
 * platform, searching, running a command (the dialog closes first), Enter,
 * a fresh search each time it opens, and the menu's forwarded shortcuts.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  commandsFor,
  matching,
} from '../../../../../../src/renderer/features/connection/view-models/palette-view-model.ts';
import { build, settle } from '../harness.ts';

describe('the command palette', () => {
  it('writes shortcuts with ⌘ on a Mac and Ctrl elsewhere', () => {
    assert.equal(commandsFor('MacIntel')[0].shortcut, '⌘ L');
    assert.equal(commandsFor('Win32')[0].shortcut, 'Ctrl L');
    assert.equal(commandsFor('Win32').find((c) => c.id === 'inspect')?.shortcut, 'Ctrl Alt I');
  });

  it('lists the commands whose label holds the search, ignoring case', () => {
    const labels = matching(commandsFor('MacIntel'), 'TAB').map((c) => c.label);
    assert.deepEqual(labels, ['New tab']);
    assert.deepEqual(matching(commandsFor('MacIntel'), 'zzz'), []);
  });

  it('closes the dialog, then runs the command', async () => {
    const app = build();
    await app.dialog.open();
    await app.palette.run('newTab');
    assert.equal(app.dialog.state.open, false);
    assert.equal(app.fake.called('newTab').length, 1);
  });

  it('opens the panel on Ask and focuses its box; on Inspect, the actions', async () => {
    const app = build();
    await app.palette.run('ask');
    assert.equal(app.panel.state.pane, 'chat');
    assert.deepEqual(app.host, ['chat']);
    await app.palette.run('inspect');
    assert.equal(app.panel.state.pane, 'actions');
  });

  it('hands the address bar and recording to their features', async () => {
    const app = build();
    await app.palette.run('address');
    await app.palette.run('record');
    assert.deepEqual(app.host, ['address', 'record']);
  });

  it('opens the account page, and checks for updates', async () => {
    const app = build({ checkForUpdates: { state: 'none', current: '1.0.0' } });
    await app.palette.run('account');
    assert.equal(app.dialog.state.page, 'profile');
    await app.palette.run('updates');
    assert.equal(app.fake.called('checkForUpdates').length, 1);
  });

  it('runs the first match on Enter, and nothing when none match', async () => {
    const app = build();
    app.palette.search('new');
    await app.palette.runFirst();
    app.palette.search('zzz');
    await app.palette.runFirst();
    assert.equal(app.fake.called('newTab').length, 1);
  });

  it('starts with an empty search each time it opens on the commands', async () => {
    const app = build();
    await app.dialog.open();
    app.palette.search('tab');
    app.dialog.close();
    await app.dialog.open();
    assert.equal(app.palette.state.query, '');
  });

  it("runs the menu's shortcuts, and ignores unknown ones", async () => {
    const app = build();
    app.fake.emit('onShellCommand', 'address');
    app.fake.emit('onShellCommand', 'record');
    app.fake.emit('onShellCommand', 'commands');
    app.fake.emit('onShellCommand', 'tools');
    app.fake.emit('onShellCommand', 'toString');
    await settle();
    assert.deepEqual(app.host, ['address', 'record']);
    assert.equal(app.dialog.state.open, true);
    assert.equal(app.fake.called('toggleDevPanel').length, 1);
  });
});

/** New entry points reuse the palette's existing view models and guarded bridge. */
describe('keyboard discovery', () => {
  it('opens help from a command and forwarded key without showing the account page', async () => {
    const app = build();
    await app.palette.run('shortcuts');
    assert.equal(app.dialog.state.page, 'shortcuts');
    assert.equal(app.dialog.state.open, true);
    app.dialog.close();
    app.fake.emit('onShellCommand', 'shortcuts');
    await settle();
    assert.equal(app.dialog.state.page, 'shortcuts');
    assert.equal(app.dialog.state.open, true);
  });
  it('opens the Library and navigates through existing IPC', async () => {
    const app = build();
    for (const id of ['library', 'back', 'forward'] as const) await app.palette.run(id);
    for (const method of ['showLibrary', 'goBack', 'goForward']) assert.equal(app.fake.called(method).length, 1);
  });
  it('opens workspace panes from forwarded shortcuts', async () => {
    const app = build();
    for (const [command, pane] of [
      ['playbooks', 'playbooks'],
      ['routines', 'routines'],
      ['inspect', 'actions'],
      ['network', 'network'],
      ['source', 'source'],
    ]) {
      app.fake.emit('onShellCommand', command);
      await settle();
      assert.equal(app.panel.state.pane, pane);
    }
  });
});

/** A guide must never remain above the workspace selected from the keyboard. */
it('dismisses shortcut help before focusing Ask Oya', async () => {
  const app = build();
  await app.dialog.openShortcuts();
  app.fake.emit('onShellCommand', 'ask');
  await settle();
  assert.equal(app.dialog.state.open, false);
  assert.equal(app.panel.state.pane, 'chat');
  assert.deepEqual(app.host, ['chat']);
});

/** Default-browser setup remains keyboard discoverable after onboarding. */
it('opens the explicit OS default-browser flow from Commands', async () => {
  const app = build();
  await app.dialog.open();
  await app.palette.run('defaultBrowser');
  assert.equal(app.dialog.state.open, false);
  assert.equal(app.fake.called('makeDefaultBrowser').length, 1);
});
