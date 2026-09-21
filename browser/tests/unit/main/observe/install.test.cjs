/**
 * Unit tests for wiring a tab into the observer (main/observe/install.cjs):
 * console messages arrive as Electron's event object and are kept by level name.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { Observer } = require('../../../../main/observe/observer.cjs');
const { watchContents } = require('../../../../main/observe/install.cjs');

describe('watchContents', () => {
  it("keeps a tab's console messages from Electron's event object", () => {
    const observer = new Observer();
    const contents = new EventEmitter();
    watchContents(observer, contents);
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
