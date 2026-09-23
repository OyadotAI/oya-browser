/**
 * Unit tests for the Ask pane's controls: Stop stands in for Send while the
 * agent works and ends the run, Clear forgets the conversation (and drops an
 * answer that arrives after it), attached files go with every later turn, and
 * the profile picker lists the project's personas and switches between them.
 */
const { describe, it, afterEach } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

/** Every renderer a test loaded, so its step-line ticker can be stopped after. */
const loaded = [];
afterEach(() => loaded.splice(0).forEach((app) => app.run('ChatProgress.stop()')));

/** A loaded renderer whose sendChat waits until the test answers it. */
async function chatting(answers = {}) {
  let answer;
  const app = loadRenderer({ answers: { sendChat: () => new Promise((resolve) => (answer = resolve)), ...answers } });
  loaded.push(app);
  await settle();
  /** Types `text` into the Ask box and presses Send. */
  const ask = async (text) => {
    app.$('chat-input').value = text;
    app.fire(app.$('chat-send'), 'click');
    await settle();
  };
  return { app, ask, answer: async (data) => (answer(data), await settle()) };
}

/** A value from the renderer's realm as a plain one, so deepEqual compares only its shape. */
const plain = (value) => (value === undefined ? undefined : JSON.parse(JSON.stringify(value)));

/** The messages shown, as "role: text". */
const shown = (app) =>
  app
    .$('chat-messages')
    .querySelectorAll('.chat-msg')
    .map((m) => `${m.classList.contains('user') ? 'user' : 'agent'}: ${m.textContent.replace(/Copy$/, '')}`);

describe('the Ask pane', () => {
  it('shows Stop instead of Send while the agent works, and Send again after', async () => {
    const { app, ask, answer } = await chatting();
    await ask('find the price');
    assert.deepEqual([app.$('chat-send').hidden, app.$('chat-stop').hidden], [true, false]);
    await answer({ text: 'It costs $5' });
    assert.deepEqual([app.$('chat-send').hidden, app.$('chat-stop').hidden], [false, true]);
  });

  it('stops the run with Stop, and says it stopped', async () => {
    const { app, ask, answer } = await chatting();
    await ask('find the price');
    app.fire(app.$('chat-stop'), 'click');
    assert.equal(app.bridge.called('stopChat').length, 1);
    await answer({ error: 'Stopped' });
    assert.deepEqual(shown(app), ['user: find the price', 'agent: Stopped.']);
  });

  it('clears the conversation, stopping a run still going and dropping its late answer', async () => {
    const { app, ask, answer } = await chatting();
    await ask('find the price');
    app.fire(app.$('chat-clear'), 'click');
    assert.equal(app.bridge.called('stopChat').length, 1);
    await answer({ error: 'Stopped' });
    assert.deepEqual(shown(app), []);
    assert.ok(app.$('chat-messages').querySelector('.chat-empty'), 'the empty state is back');
    await ask('hello');
    assert.deepEqual(plain(app.bridge.called('sendChat').at(-1)[0]), [{ role: 'user', content: 'hello' }]);
  });

  it('sends attached files with the message that took them, and with every later turn', async () => {
    const { app, ask, answer } = await chatting();
    app.run(
      "ChatFiles.read = async (f) => f.b64; ChatFiles.add([{ name: 'cv.pdf', type: 'application/pdf', b64: 'YQ==' }])",
    );
    await settle();
    assert.equal(app.$('chat-files').hidden, false);
    assert.match(app.$('chat-files').textContent, /cv\.pdf/);
    await ask('upload my CV');
    const file = { file: 'cv.pdf', type: 'application/pdf', b64: 'YQ==' };
    const [messages, data] = app.bridge.called('sendChat')[0];
    assert.deepEqual(plain(data), { file1: file });
    assert.equal(messages[0].content, 'upload my CV\n\n(Attached: cv.pdf)');
    assert.equal(app.$('chat-files').hidden, true, 'the chips go once sent');
    await answer({ text: 'Uploaded' });
    await ask('now submit');
    assert.deepEqual(plain(app.bridge.called('sendChat')[1][1]), { file1: file });
  });

  it('refuses a file past the size cap, with a note, and lets a pending one be removed', async () => {
    const { app } = await chatting();
    app.run("huge = 'A'.repeat(RendererConstants.CHAT_FILES_MAX_B64 + 4)");
    app.run(
      "ChatFiles.read = async (f) => f.b64; ChatFiles.add([{ name: 'a.txt', b64: 'YQ==' }, { name: 'big.iso', b64: huge }])",
    );
    await settle();
    assert.match(app.$('chat-files').textContent, /up to 10 MB/);
    assert.equal(app.run('ChatFiles.pending.length'), 1);
    app.fire(app.$('chat-files').querySelector('.chat-file-remove'), 'click');
    assert.equal(app.run('ChatFiles.pending.length'), 0);
    assert.equal(app.$('chat-files').hidden, true);
  });

  it('sends no data when nothing is attached', async () => {
    const { app, ask } = await chatting();
    await ask('hi');
    assert.equal(app.bridge.called('sendChat')[0][1], undefined);
  });
});

describe('the profile picker', () => {
  const personas = [
    { id: 'd', name: 'Default', isDefault: true },
    { id: 'p-work', name: 'Work', isDefault: false },
  ];

  it('fills itself at load, for a window that opened after the browser connected', async () => {
    const { app } = await chatting({ listPersonas: { personas, active: 'default' } });
    assert.equal(app.$('chat-persona').querySelectorAll('option').length, 2);
    assert.equal(app.$('chat-persona').disabled, false);
  });

  it('is locked until the browser is connected', async () => {
    const { app } = await chatting();
    assert.equal(app.$('chat-persona').disabled, true);
  });

  it('lists the named personas once connected, with the one in use chosen', async () => {
    const { app } = await chatting({ listPersonas: { personas, active: 'p-work' } });
    app.bridge.emit('WsStatus', { connected: true });
    await settle();
    const select = app.$('chat-persona');
    assert.deepEqual(
      select.querySelectorAll('option').map((o) => [o.value, o.textContent]),
      [
        ['default', 'Default profile'],
        ['p-work', 'Work'],
      ],
    );
    assert.equal(select.value, 'p-work');
    assert.equal(select.disabled, false);
  });

  it('reconnects as the chosen persona, locked until the switch lands', async () => {
    const { app } = await chatting({ listPersonas: { personas, active: 'default' } });
    app.bridge.emit('WsStatus', { connected: true });
    await settle();
    app.$('chat-persona').value = 'p-work';
    app.fire(app.$('chat-persona'), 'change');
    assert.deepEqual(plain(app.bridge.called('saveConfig').at(-1)), [{ persona: 'p-work' }]);
    assert.equal(app.$('chat-persona').disabled, true);
    app.bridge.emit('FingerprintChanged', {});
    await settle();
    assert.equal(app.$('chat-persona').disabled, false);
  });

  it('cannot be changed while the agent works, or once offline', async () => {
    const { app, ask, answer } = await chatting({ listPersonas: { personas, active: 'default' } });
    app.bridge.emit('WsStatus', { connected: true });
    await settle();
    await ask('go');
    assert.equal(app.$('chat-persona').disabled, true);
    await answer({ text: 'done' });
    assert.equal(app.$('chat-persona').disabled, false);
    app.bridge.emit('WsStatus', { connected: false });
    assert.equal(app.$('chat-persona').disabled, true);
  });
});
