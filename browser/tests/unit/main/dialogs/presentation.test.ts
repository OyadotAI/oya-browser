/** Site-controlled text never becomes markup, a trusted heading or a credential-bearing address. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { dialogPresentation } from '../../../../src/main/dialogs/presentation.ts';
/** Shape one native engine event with deliberately untrusted page text. */
const info = (url: string, dialogType = 'prompt') => ({
  frame: { url },
  dialogType,
  messageText: '<script>attack()</script>',
  defaultPromptText: 'secret default',
});
test('presentation excludes URL credentials, path, query and fragment', () => {
  const data = dialogPresentation(info('https://user:password@example.test/a?secret=1#token'));
  assert.equal(data.origin, 'https://example.test');
  assert.equal(data.message, '<script>attack()</script>');
  assert.equal(data.value, 'secret default');
});
test('opaque and local frames do not disclose paths or page-selected origins', () => {
  for (const url of ['file:///private/sensitive.txt', 'data:text/html,secret', 'bad url'])
    assert.equal(dialogPresentation(info(url)).origin, 'This page');
});
test('unload warning is browser-owned with a safe default', () => {
  const data = dialogPresentation({ ...info('https://site.test', 'beforeunload'), isReload: true });
  assert.equal(data.title, 'Leave this page?');
  assert.equal(data.accept, 'Reload page');
  assert.equal(data.cancel, 'Stay on page');
  assert.equal(data.prompt, false);
  assert.equal(data.message, 'Changes you made may not be saved.');
});
