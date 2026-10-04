/**
 * Unit tests for wiring a tab into the observer (src/main/observe/install.ts):
 * console messages arrive as Electron's event object and are kept by level name.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { Observer } from '../../../../src/main/observe/observer.ts';
import { watchContents } from '../../../../src/main/observe/install.ts';

describe('watchContents', () => {
  it("keeps a tab's console messages from Electron's event object", () => {
    const observer = new Observer();
    const contents = new EventEmitter();
    watchContents(observer, contents as any);
    contents.emit('console-message', {
      level: 'error',
      message: 'boom',
      lineNumber: 7,
      sourceId: 'https://a.test/app.js',
    });
    const [entry] = observer.readConsole();
    assert.deepEqual([entry.level, entry.message, entry.line], ['error', 'boom', 7]);
  });
});
