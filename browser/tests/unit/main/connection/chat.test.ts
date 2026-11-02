/**
 * Unit tests for what a chat sends the server: the tab's lane, the files when
 * there are any, and remember: false only for a run that is not part of the
 * person's chat (a routine), so the server leaves that tab's chat memory alone;
 * and no agent is ever handed an incognito tab.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { Chat } from '../../../../src/main/connection/chat.ts';
import { ServerApi } from '../../../../src/main/connection/server-api.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

/** A chat whose server calls are recorded instead of sent, with its in-tab and one-at-a-time steps passed through. */
function recordingChat(t) {
  const sent: unknown[] = [];
  t.mock.method(ServerApi.prototype, 'canCall', () => true);
  t.mock.method(Chat.prototype as any, 'asOnlyChat', (_tab, work) => work(new AbortController().signal));
  t.mock.method(Chat.prototype as any, 'inTab', (_tab, work) => work());
  t.mock.method(Chat.prototype, 'ask', async (_route, payload) => (sent.push(payload), { text: 'DONE' }));
  return { chat: new Chat({ tabs: { activeTabId: 1 } } as any), sent };
}

it('a chat from a tab is sent in that tab’s lane, remembered by default', async (t) => {
  const { chat, sent } = recordingChat(t);
  await chat.send([{ role: 'user', content: 'go' }], undefined, 3);
  assert.deepEqual(sent, [{ messages: [{ role: 'user', content: 'go' }], lane: 'tab-3' }]);
});

it('a run that is not part of the person’s chat asks the server not to remember it', async (t) => {
  const { chat, sent } = recordingChat(t);
  await chat.send([{ role: 'user', content: 'routine' }], undefined, 3, false);
  assert.deepEqual(sent, [{ messages: [{ role: 'user', content: 'routine' }], lane: 'tab-3', remember: false }]);
});

it('attached files go with the chat', async (t) => {
  const { chat, sent } = recordingChat(t);
  await chat.send([{ role: 'user', content: 'go' }], { cv: 'file' });
  assert.deepEqual(sent, [{ messages: [{ role: 'user', content: 'go' }], data: { cv: 'file' }, lane: 'tab-1' }]);
});

it('chat from an incognito tab is refused and claims nothing', async () => {
  const ctx = mainCtx();
  const incognitoScope = { tabs: { privateSession: {} } };
  ctx.windows = { owner: (id: number) => (id === 7 ? incognitoScope : undefined) };
  ctx.agentTab.claim = () => assert.fail('an incognito tab must never be claimed');
  const answer = await new Chat(ctx).send([{ role: 'user', content: 'hi' }], undefined, 7);
  assert.match(answer.error, /can't use incognito/);
});
