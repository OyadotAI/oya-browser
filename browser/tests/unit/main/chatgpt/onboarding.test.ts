/**
 * Unit tests for first-run onboarding: a browser without a key gets one from its
 * ChatGPT email and connects (importing logins when asked), the project's model
 * moves to this desktop either way, and no email means the Oya sign-in instead.
 */
import { it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Onboarding } from '../../../../src/main/chatgpt/onboarding.ts';

/** Onboarding over fakes; `apiKey` is the browser's key before it starts. */
function setup(apiKey = '', email = 'me@x.test') {
  const values: Record<string, unknown> = { apiKey, serverUrl: 'wss://oya.test/ws' };
  const config = { values, merge: (c: object) => Object.assign(values, c), save() {} };
  const socket = {
    browserId: 'old',
    connected: 0,
    ready: true,
    connect() {
      this.connected++;
    },
  };
  const mirror = { importOnConnect: false };
  const chatgpt = {
    signIn: mock.fn(async () => ({ connected: true, email })),
    status: () => ({ connected: true, email }),
  };
  const electron = { app: { isDefaultProtocolClient: () => false } };
  const signUp = mock.fn(async () => ({ apiKey: 'new-key', claimUrl: 'https://oya.test/claim#k' }));
  const onboarding = new Onboarding({ config, socket, mirror, chatgpt, electron, tabs: {}, shell: {} } as any, signUp);
  return { onboarding, values, socket, mirror, signUp, chatgpt };
}

afterEach(() => mock.restoreAll());

it('canceled signup cannot save a late key or configure a model', async () => {
  const { onboarding, signUp, values } = setup();
  const answer = Promise.withResolvers<any>();
  signUp.mock.mockImplementation(() => answer.promise);
  const fetched = mock.method(globalThis, 'fetch', async () => new Response('{}'));
  const pending = onboarding.connect({ importLogins: false });
  await new Promise((resolve) => setImmediate(resolve));
  onboarding.cancel();
  answer.resolve({ apiKey: 'late-key', claimUrl: '' });
  await assert.rejects(pending);
  assert.equal(values.apiKey, '');
  assert.equal(fetched.mock.callCount(), 0);
});
it('a workspace change during signup cannot overwrite the newer destination', async () => {
  const { onboarding, signUp, values } = setup();
  const answer = Promise.withResolvers<any>();
  signUp.mock.mockImplementation(() => answer.promise);
  const pending = onboarding.connect({ importLogins: false });
  await new Promise((resolve) => setImmediate(resolve));
  Object.assign(values, { serverUrl: 'wss://new.test/ws', apiKey: 'new-workspace' });
  answer.resolve({ apiKey: 'old-workspace', claimUrl: '' });
  await assert.rejects(pending, /workspace changed/);
  assert.equal(values.apiKey, 'new-workspace');
});
it('retry after cancellation of a first model write connects the already-minted key', async () => {
  const { onboarding, socket } = setup();
  socket.ready = false;
  const response = Promise.withResolvers<Response>();
  const fetched = mock.method(globalThis, 'fetch', () => response.promise);
  const first = onboarding.connect({ importLogins: false });
  await new Promise((resolve) => setImmediate(resolve));
  onboarding.cancel();
  response.resolve(new Response('{}'));
  await assert.rejects(first);
  fetched.mock.mockImplementation(async () => new Response('{}'));
  await onboarding.connect({ importLogins: false });
  assert.equal(socket.connected, 1);
});
it('saved credentials alone do not report a connected browser', () => {
  const { onboarding, socket } = setup('stored');
  socket.ready = false;
  onboarding.begin();
  assert.equal(onboarding.active, true);
  assert.equal(onboarding.status().connected, false);
  onboarding.reset();
  assert.equal(onboarding.active, false);
});

it('makes a key from the ChatGPT email, puts the model on this desktop, and connects importing the logins', async () => {
  const posts: any[] = [];
  mock.method(
    globalThis,
    'fetch',
    async (url: string, init: RequestInit) => (posts.push([url, JSON.parse(String(init.body))]), new Response('{}')),
  );
  const { onboarding, values, socket, mirror, signUp } = setup();
  await onboarding.connect({ importLogins: true });
  assert.deepEqual(signUp.mock.calls[0].arguments.slice(0, 2), ['wss://oya.test/ws', 'me@x.test']);
  assert.deepEqual([values.apiKey, values.claimUrl], ['new-key', 'https://oya.test/claim#k']);
  assert.equal(posts[0][0], 'https://oya.test/api/config');
  assert.deepEqual([posts[0][1].llm_provider, posts[0][1].openai_api_key], ['chatgpt', 'chatgpt-desktop']);
  assert.deepEqual(
    [socket.connected, socket.browserId, mirror.importOnConnect, onboarding.active],
    [1, null, true, true],
  );
});

it('a browser that already has a key only moves its model, without signing up or reconnecting', async () => {
  mock.method(globalThis, 'fetch', async () => new Response('{}'));
  const { onboarding, socket, signUp } = setup('existing');
  await onboarding.connect({ importLogins: false });
  assert.deepEqual([signUp.mock.callCount(), socket.connected, onboarding.active], [0, 0, false]);
});

it('without an email from ChatGPT, sends the person to the Oya sign-in', async () => {
  const { onboarding, signUp } = setup('', '');
  await assert.rejects(onboarding.connect({ importLogins: true }), /sign in instead/);
  assert.equal(signUp.mock.callCount(), 0);
});

it('a retry after a later step failed starts from the ChatGPT sign-in that worked, not from OpenAI again', async () => {
  const { onboarding, signUp, chatgpt } = setup();
  signUp.mock.mockImplementationOnce(async () => {
    throw Error('Too many sign-ups');
  });
  await assert.rejects(onboarding.connect({ importLogins: false }), /Too many/);
  mock.method(globalThis, 'fetch', async () => new Response('{}'));
  await onboarding.connect({ importLogins: false });
  assert.equal(chatgpt.signIn.mock.callCount(), 1);
  await onboarding.connect({ importLogins: false });
  assert.equal(chatgpt.signIn.mock.callCount(), 2, 'a finished connect forgets it, so the next is a fresh sign-in');
});

it('the Gemini key route makes a key from the typed email, puts the project on Gemini, and connects', async () => {
  const posts: any[] = [];
  mock.method(globalThis, 'fetch', async (_url: string, init: RequestInit) => {
    posts.push(JSON.parse(String(init.body)));
    return new Response('{}');
  });
  const { onboarding, values, socket, mirror, signUp, chatgpt } = setup();
  await onboarding.connectWithKey({ email: ' me@y.test ', key: ' AIza1 ', importLogins: true });
  assert.deepEqual(signUp.mock.calls[0].arguments.slice(0, 2), ['wss://oya.test/ws', 'me@y.test']);
  assert.deepEqual([posts[0].llm_provider, posts[0].openai_api_key, posts[0].chat_model], ['gemini', 'AIza1', null]);
  assert.deepEqual([socket.connected, mirror.importOnConnect, values.modelFromKey], [1, true, true]);
  assert.equal(chatgpt.signIn.mock.callCount(), 0);
});

it('the Gemini key route keeps an existing Oya key and does not reconnect', async () => {
  mock.method(globalThis, 'fetch', async () => new Response('{}'));
  const { onboarding, socket, signUp } = setup('existing');
  await onboarding.connectWithKey({ email: 'me@y.test', key: 'AIza1', importLogins: false });
  assert.deepEqual([signUp.mock.callCount(), socket.connected], [0, 0]);
});

it('the Gemini key route refuses a bad email or an empty key before calling anything', async () => {
  const fetched = mock.method(globalThis, 'fetch', async () => new Response('{}'));
  const { onboarding, signUp } = setup();
  await assert.rejects(onboarding.connectWithKey({ email: 'nope', key: 'AIza1', importLogins: false }), /email/);
  await assert.rejects(onboarding.connectWithKey({ email: 'me@y.test', key: ' ', importLogins: false }), /Gemini/);
  assert.deepEqual([signUp.mock.callCount(), fetched.mock.callCount()], [0, 0]);
});

it('the project has a model once a Gemini key is in, without a ChatGPT sign-in', async () => {
  mock.method(globalThis, 'fetch', async () => new Response('{}'));
  const { onboarding, chatgpt } = setup('existing');
  chatgpt.status = () => ({ connected: false, email: '' });
  assert.equal(onboarding.status().hasModel, false);
  await onboarding.connectWithKey({ email: 'me@y.test', key: 'AIza1', importLogins: false });
  assert.equal(onboarding.status().hasModel, true);
});

it('a new Oya key forgets that an old one had a Gemini model', async () => {
  mock.method(globalThis, 'fetch', async () => new Response('{}'));
  const { onboarding, values } = setup();
  values.modelFromKey = true;
  await onboarding.connect({ importLogins: false });
  assert.equal(values.modelFromKey, false);
});
