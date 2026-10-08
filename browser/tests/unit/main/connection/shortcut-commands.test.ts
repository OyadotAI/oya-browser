/** Shortcut discovery matches the human guide and works without a page. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { keyboardShortcuts } from '../../../../src/main/connection/shortcut-commands.ts';
import { SHORTCUTS, shortcutLabel } from '../../../../src/shared/shortcuts.ts';
import { CommandRunner } from '../../../../src/main/connection/commands.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

it('reports every live binding for the browser platform, not the server platform', () => {
  for (const platform of ['darwin', 'win32', 'linux']) {
    const result = keyboardShortcuts(platform);
    assert.equal(result.platform, platform);
    assert.equal(result.scope, 'browser-shell');
    assert.equal(result.shortcuts.length, SHORTCUTS.length);
    result.shortcuts.forEach((row, index) => {
      assert.equal(row.command, SHORTCUTS[index][0]);
      assert.equal(row.shortcut, shortcutLabel(SHORTCUTS[index], platform));
    });
    assert.match(result.guidance, /not a shell-shortcut execution API/);
  }
});
it('returns bindings through the normal command envelope without a tab or focus change', async () => {
  const ctx = mainCtx({ commands: CommandRunner });
  await ctx.commands.handleCommand({ id: 'keys', action: 'list_keyboard_shortcuts', params: {} });
  const result = ctx.socket.ofType('cmd_result').at(-1);
  assert.equal(result.ok, true);
  assert.deepEqual(result.data, keyboardShortcuts(process.platform));
  assert.deepEqual(ctx.shell.sentOn('shell-command'), []);
});
