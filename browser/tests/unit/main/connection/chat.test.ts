/**
 * Unit tests for what a chat sends the server: the tab's lane, the files when
 * there are any, and remember: false only for a run that is not part of the
 * person's chat (a routine), so the server leaves that tab's chat memory alone;
 * and no agent is ever handed an incognito tab. Then what a chat does when its
 * connection drops mid-run: it comes back for the same run, Stop still stops,
 * and a run it cannot collect is told to stop.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { Chat } from '../../../../src/main/connection/chat.ts';
import { ServerApi } from '../../../../src/main/connection/server-api.ts';
import { mainCtx } from '../../support/main-ctx.cjs';
import { REATTACH_RETRY_MS } from '../../../../src/main/connection/constants.ts';

/** A chat whose server calls are recorded instead of sent, with its in-tab and one-at-a-time steps passed through. */
function recordingChat(t) {
  const sent: unknown[] = [];
  t.mock.method(ServerApi.prototype, 'canCall', () => true);
  t.mock.method(Chat.prototype as any, 'asOnlyChat', (_tab, work) => work(new AbortController().signal));
  t.mock.method(Chat.prototype as any, 'inTab', (_tab, work) => work());
  const record = ({ resume, ...said }) => (assert.match(resume, /^[A-Za-z0-9._-]{1,64}$/), sent.push(said));
  t.mock.method(Chat.prototype, 'ask', async (_route, payload) => (record(payload), { text: 'DONE' }));
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

/** A response whose body is cut while it is read, as a dropped connection cuts it. */
function cutResponse() {
  const body = new ReadableStream({ start: (c) => c.error(new TypeError('terminated')) });
  return new Response(body, { status: 200 });
}

/** A JSON answer with `status`. */
const answered = (body: object, status = 200) => new Response(JSON.stringify(body), { status });

/**
 * A chat in tab 3 whose server calls go to `server(route, n)`, with what it told the panel and the
 * routes it called. `connected` false is a socket that is still down when the chat comes back.
 */
function droppedChat(t, server: (route: string, signal?: AbortSignal) => Promise<Response> | Response) {
  const abort = new AbortController();
  const told: string[] = [];
  const routes: string[] = [];
  const bodies: any[] = [];
  t.mock.method(Chat.prototype as any, 'asOnlyChat', (_tab, work) => work(abort.signal));
  t.mock.method(Chat.prototype as any, 'inTab', (_tab, work) => work());
  t.mock.method(globalThis, 'fetch', async (url: string, init: RequestInit) => {
    const route = url.split('/oya-1/')[1];
    routes.push(route);
    bodies.push(JSON.parse(String(init.body)));
    return server(route, init.signal ?? undefined);
  });
  const socket = { ready: true, browserId: 'oya-1' };
  const shell = { send: (_channel: string, payload: any) => told.push(payload.event.kind) };
  const config = { values: { serverUrl: 'ws://server.test/ws', apiKey: 'k' } };
  const chat = new Chat({ tabs: { activeTabId: 3 }, socket, shell, config } as any);
  return { chat, abort, told, routes, bodies, socket };
}

/** The chat's question. */
const ASKED = [{ role: 'user', content: 'go' }];

it('a chat whose connection drops mid-run comes back for the same run and answers with its real answer', async (t) => {
  const { chat, told, routes, bodies, socket } = droppedChat(t, (route) => {
    if (route === 'chat') return ((socket.ready = false), cutResponse());
    return answered({ text: 'the real answer' });
  });
  const answer = await chat.send(ASKED);
  assert.deepEqual(answer, { text: 'the real answer' });
  assert.deepEqual(routes, ['chat', 'chat/resume']);
  assert.deepEqual(told, ['reconnecting', 'resumed']);
  assert.deepEqual(bodies[1], { lane: 'tab-3', resume: bodies[0].resume });
});

it('an answer cut a second time is collected again, since the run is still going', async (t) => {
  let tries = 0;
  const { chat, routes } = droppedChat(t, (route) => {
    if (route === 'chat' || tries++ === 0) return cutResponse();
    return answered({ text: 'third time' });
  });
  assert.deepEqual(await chat.send(ASKED), { text: 'third time' });
  assert.deepEqual(routes, ['chat', 'chat/resume', 'chat/resume']);
});

it('a run the server no longer has is an error that names no restart, and the run is told to stop', async (t) => {
  const gone = { error: 'No chat run to resume', code: 'chat_run_gone' };
  const { chat, routes } = droppedChat(t, (route) => (route === 'chat' ? cutResponse() : answered(gone, 404)));
  const answer = await chat.send(ASKED);
  assert.match(answer.error, /could not be resumed/);
  assert.doesNotMatch(answer.error, /restart/);
  assert.deepEqual(routes, ['chat', 'chat/resume', 'chat/stop']);
});

it('Stop while coming back answers Stopped at once and tells the server to stop the run', async (t) => {
  const { chat, abort, routes } = droppedChat(t, (route, signal) => {
    if (route === 'chat') return cutResponse();
    if (route === 'chat/stop') return answered({ ok: true, stopped: true });
    queueMicrotask(() => abort.abort());
    return new Promise((_resolve, reject) => signal.addEventListener('abort', () => reject(signal.reason)));
  });
  assert.deepEqual(await chat.send(ASKED), { error: 'Stopped' });
  assert.deepEqual(routes, ['chat', 'chat/resume', 'chat/stop']);
});

it('Stop during the run itself tells the server too, since hanging up no longer stops a run that can be resumed', async (t) => {
  const { chat, abort, routes } = droppedChat(t, (route, signal) => {
    if (route === 'chat/stop') return answered({ ok: true, stopped: true });
    queueMicrotask(() => abort.abort());
    return new Promise((_resolve, reject) => signal.addEventListener('abort', () => reject(signal.reason)));
  });
  assert.deepEqual(await chat.send(ASKED), { error: 'Stopped' });
  assert.deepEqual(routes, ['chat', 'chat/stop']);
});

it('a chat that answers normally calls nothing else', async (t) => {
  const { chat, routes, told } = droppedChat(t, () => answered({ text: 'DONE' }));
  assert.deepEqual(await chat.send(ASKED), { text: 'DONE' });
  assert.deepEqual(routes, ['chat']);
  assert.deepEqual(told, []);
});

it('a stop names the run, so one that lands late cannot end the next run in the tab', async (t) => {
  const gone = { error: 'No chat run to resume', code: 'chat_run_gone' };
  const { chat, bodies } = droppedChat(t, (route) => (route === 'chat' ? cutResponse() : answered(gone, 404)));
  await chat.send(ASKED);
  assert.deepEqual(bodies[2], { lane: 'tab-3', resume: bodies[0].resume });
});

it('a chat that never reached the server is told to stop too, in case the server did hear it', async (t) => {
  const { chat, routes } = droppedChat(t, (route) => {
    if (route === 'chat') throw new TypeError('fetch failed');
    return answered({ ok: true, stopped: false });
  });
  assert.deepEqual(await chat.send(ASKED), { error: 'fetch failed' });
  assert.deepEqual(routes, ['chat', 'chat/stop']);
});

it('a stop the server cannot hear is sent again until it does', async (t) => {
  let stops = 0;
  const { chat, routes } = droppedChat(t, (route) => {
    if (route === 'chat') throw new TypeError('fetch failed');
    if (stops++ === 0) throw new TypeError('fetch failed');
    return answered({ ok: true, stopped: true });
  });
  await chat.send(ASKED);
  await new Promise((resolve) => setTimeout(resolve, REATTACH_RETRY_MS + 200));
  assert.deepEqual(routes, ['chat', 'chat/stop', 'chat/stop']);
});
