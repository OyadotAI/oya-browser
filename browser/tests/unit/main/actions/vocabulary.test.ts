/**
 * Unit tests for the actions this app announces: exactly what its command
 * maps do, sorted and without the server's internal ones, so the server's
 * check and the browser detail never disagree with the app.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { OYA_ACTIONS } from '../../../../src/main/actions/vocabulary.ts';
import { PAGE_COMMANDS } from '../../../../src/main/actions/page-commands.ts';
import { ACTION_SCRIPTS } from '../../../../src/main/actions/scripts.ts';
import { TAB_COMMANDS } from '../../../../src/main/connection/tab-commands.ts';

/** Actions only the server sends: done by the app, never announced. */
const INTERNAL = new Set(['evaluate_raw', 'record', 'run_script']);

describe('OYA_ACTIONS', () => {
  it('equals the actions the command maps answer, plus handle_dialog, without the internal ones', () => {
    const maps = [PAGE_COMMANDS, TAB_COMMANDS, ACTION_SCRIPTS];
    const answered = new Set(['handle_dialog', ...maps.flatMap((map) => Object.keys(map))]);
    assert.deepEqual(OYA_ACTIONS, [...answered].filter((a) => !INTERNAL.has(a)).sort());
  });
});
