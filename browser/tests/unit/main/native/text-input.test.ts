/** Composed text requires the exact acknowledged engine capability; no legacy or page-script fallback. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { insertNativeText } from '../../../../src/main/native/text-input.ts';
test('text insertion refuses unsupported and destroyed engines and awaits native acknowledgement', async () => {
  const sent: string[] = [];
  let destroyed = false,
    acknowledge: () => void = () => {};
  const page = {
    webContents: {
      isDestroyed: () => destroyed,
      insertText: async () => {
        throw Error('legacy text fallback forbidden');
      },
      _insertTextOya: undefined as undefined | ((text: string) => Promise<void>),
    },
  };
  await assert.rejects(insertNativeText(page, 'hidden'), /lacks acknowledged/);
  page.webContents._insertTextOya = async (text) => {
    sent.push(text);
    await new Promise<void>((resolve) => {
      acknowledge = resolve;
    });
  };
  let completed = false;
  const task = insertNativeText(page, '日本語 🙂').then(() => {
    completed = true;
  });
  await Promise.resolve();
  assert.equal(completed, false);
  acknowledge();
  await task;
  assert.equal(completed, true);
  assert.deepEqual(sent, ['日本語 🙂']);
  destroyed = true;
  await assert.rejects(insertNativeText(page, 'gone'), /destroyed/);
  assert.deepEqual(sent, ['日本語 🙂']);
});
test('native cancellation is an error, never a successful empty insertion', async () => {
  const page = {
    webContents: {
      isDestroyed: () => false,
      _insertTextOya: async () => {
        throw Error('Text input document was replaced');
      },
    },
  };
  await assert.rejects(insertNativeText(page, 'text'), /document was replaced/);
});
