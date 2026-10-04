/**
 * Unit tests for the Ask pane's conversation: Stop stands in for Send while
 * the agent works and ends the run, Clear forgets the conversation (and drops
 * an answer that arrives after it), files go with every later turn, every
 * kind of answer shows as it should, and Save as playbook is offered under
 * the newest reply only.
 */
import { describe, it, mock, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import type {
  ChatItem,
  ChatMessage,
  RunItem,
} from '../../../../../../src/renderer/features/ask/view-models/ask-view-model.ts';
import { chatting, disposeAll, file, settle, shown } from '../support.ts';

afterEach(() => (disposeAll(), mock.timers.reset()));

/** The messages that carry an offer. */
const offers = (items: readonly ChatItem[]) =>
  (items.filter((i) => i.kind === 'message') as ChatMessage[]).filter((m) => m.offer);

describe('AskViewModel', () => {
  it('shows Stop instead of Send while the agent works, and Send again after', async () => {
    const { ask, send, answer } = await chatting();
    await send('find the price');
    assert.equal(ask.state.sending, true);
    assert.equal(ask.state.input, '', 'the box empties once sent');
    await answer({ text: 'It costs $5' });
    assert.equal(ask.state.sending, false);
    assert.deepEqual(shown(ask), ['user: find the price', 'assistant: It costs $5']);
  });

  it('stops the run with Stop, and says it stopped', async () => {
    const { fake, ask, send, answer } = await chatting();
    await send('find the price');
    ask.stop();
    assert.equal(fake.called('stopChat').length, 1);
    await answer({ error: 'Stopped' });
    assert.deepEqual(shown(ask), ['user: find the price', 'assistant: Stopped.']);
  });

  it('clears the conversation, stopping a run still going and dropping its late answer', async () => {
    const { fake, ask, send, answer } = await chatting();
    await send('find the price');
    ask.clear();
    assert.equal(fake.called('stopChat').length, 1);
    await answer({ error: 'Stopped' });
    assert.deepEqual(ask.state.items, []);
    assert.equal(ask.run.state.card, null);
    await send('hello');
    assert.deepEqual(fake.called('sendChat').at(-1)?.[0], [{ role: 'user', content: 'hello' }]);
  });

  it('is what Clear does while the panel shows Ask', async () => {
    const { panel, ask, send, answer } = await chatting();
    await send('hi');
    await answer({ text: 'hello' });
    panel.clear();
    assert.deepEqual(ask.state.items, []);
  });

  it('sends the whole conversation, errors included, with each question', async () => {
    const { fake, send, answer } = await chatting();
    await send('one');
    await answer({ error: 'boom' });
    await send('two');
    assert.deepEqual(fake.called('sendChat').at(-1)?.[0], [
      { role: 'user', content: 'one' },
      { role: 'assistant', content: 'Error: boom' },
      { role: 'user', content: 'two' },
    ]);
  });

  it('sends attached files with the message that took them, and with every later turn', async () => {
    const { fake, ask, send, answer } = await chatting();
    await ask.files.add([file('cv.pdf', 'YQ==', 'application/pdf')]);
    await send('upload my CV');
    const attached = { file: 'cv.pdf', type: 'application/pdf', b64: 'YQ==' };
    const [messages, data] = fake.called('sendChat')[0] as [ChatMessage[], unknown];
    assert.deepEqual(data, { file1: attached });
    assert.equal(messages[0].content, 'upload my CV\n\n(Attached: cv.pdf)');
    assert.deepEqual(ask.files.state.pending, [], 'the chips go once sent');
    await answer({ text: 'Uploaded' });
    await send('now submit');
    assert.deepEqual(fake.called('sendChat')[1][1], { file1: attached });
  });

  it('sends no data when nothing is attached', async () => {
    const { fake, send } = await chatting();
    await send('hi');
    assert.equal(fake.called('sendChat')[0][1], undefined);
  });

  it('ignores an empty question, and any asked while one is on its way', async () => {
    const { fake, ask, send } = await chatting();
    await send('   ');
    await send('first');
    await ask.ask('second');
    assert.equal(fake.called('sendChat').length, 1);
  });

  it('opens the panel on Ask when another feature hands it a task', async () => {
    const { fake, panel, ask } = await chatting();
    panel.show('routines');
    void ask.ask('  find flights  ');
    await settle();
    assert.equal(panel.state.pane, 'chat');
    assert.equal(fake.called('toggleDevPanel').length, 1);
    assert.deepEqual(shown(ask), ['user: find flights']);
  });

  it('shows a failed call as an error reply', async () => {
    const { ask } = await chatting({ sendChat: () => Promise.reject(new Error('Not connected to server')) });
    await ask.ask('hi');
    assert.deepEqual(shown(ask), ['user: hi', 'assistant: Error: Not connected to server']);
    assert.equal(ask.state.sending, false);
  });

  it('says there was no response when the reply is empty', async () => {
    const { ask } = await chatting({ sendChat: {} });
    await ask.ask('hi');
    assert.deepEqual(shown(ask), ['user: hi', 'assistant: (no response)']);
  });

  it('asks for a model key instead of showing an error, and sends the question again once it is saved', async () => {
    const answers = [{ error: 'No LLM key', code: 'llm_unconfigured' }, { text: 'Done' }];
    const { fake, ask } = await chatting({ sendChat: () => answers.shift(), saveModelKey: { ok: true } });
    await ask.ask('find flights');
    assert.equal(ask.model.state.open, true);
    assert.deepEqual(shown(ask), ['user: find flights']);
    ask.model.setKey('sk-ant-1');
    await ask.model.save();
    await settle();
    assert.equal(fake.called('sendChat').length, 2);
    assert.deepEqual(shown(ask), ['user: find flights', 'assistant: Done']);
  });

  it('reopens the model card with the reason when the provider refuses the key', async () => {
    const refused = { error: 'Your AI provider refused the request (401).', code: 'llm_rejected' };
    const { ask } = await chatting({ sendChat: refused });
    await ask.ask('find flights');
    assert.equal(ask.model.state.open, true);
    assert.equal(ask.model.state.error, refused.error);
    assert.deepEqual(shown(ask), ['user: find flights']);
  });

  it('offers Save as playbook under the newest reply only, named from the prompt', async () => {
    const { ask } = await chatting({ sendChat: { text: 'ok', toolCalls: [{ name: 'click' }] } });
    await ask.ask('Check my inbox!');
    await ask.ask('again');
    const [offer] = offers(ask.state.items);
    assert.equal(offers(ask.state.items).length, 1);
    assert.equal(offer.content, 'ok');
    assert.equal(offer.offer?.state.name, 'again');
    assert.equal(ask.state.items.at(-1), offer);
  });

  it('keeps a saved offer after later runs', async () => {
    const { ask } = await chatting({ sendChat: { text: 'ok', replayable: true }, saveChatPlaybook: { ok: true } });
    await ask.ask('first');
    await offers(ask.state.items)[0].offer?.save();
    await ask.ask('second');
    assert.deepEqual(
      offers(ask.state.items).map((m) => m.offer?.state.stage),
      ['saved', 'offer'],
    );
  });

  it('follows the server on whether a run can be saved, and judges by the tools when it does not say', async () => {
    const { ask } = await chatting();
    ask.reply('a', [{ name: 'click' }], false);
    assert.equal(offers(ask.state.items)[0].offer?.state.canSave, false);
    ask.reply('b', [{ name: 'navigate' }], true);
    assert.equal(offers(ask.state.items)[0].offer?.state.canSave, true);
    ask.reply('c', [{ name: 'analyze_page' }, { name: 'type' }]);
    assert.equal(offers(ask.state.items)[0].offer?.state.canSave, true);
    assert.equal(offers(ask.state.items)[0].offer?.state.name, 'agent-run', 'no prompt to name it from');
  });

  it('folds the run into the conversation before the reply', async () => {
    const { fake, ask, send, answer } = await chatting();
    await send('find jordans');
    fake.emit('onAgentEvent', { runId: 'r1', event: { kind: 'start' } });
    fake.emit('onAgentEvent', { runId: 'r1', event: { kind: 'step', line: 'Opening amazon.com' } });
    await answer({ text: 'DONE: found' });
    assert.deepEqual(
      ask.state.items.map((i) => i.kind),
      ['message', 'run', 'message'],
    );
    const run = ask.state.items[1];
    assert.ok(run.kind === 'run' && run.folded && run.run.outcome === 'done');
    ask.toggleRun(run.id);
    assert.equal((ask.state.items[1] as RunItem).folded, false);
  });

  it('marks a reply that reports FAILED as a failed run', async () => {
    const { fake, ask, send, answer } = await chatting();
    await send('buy it');
    fake.emit('onAgentEvent', { runId: 'r1', event: { kind: 'start' } });
    fake.emit('onAgentEvent', { runId: 'r1', event: { kind: 'step', line: 'Clicking Buy' } });
    await answer({ text: 'FAILED: out of stock' });
    assert.equal(ask.run.state.orb, 'failed');
    assert.ok(ask.state.items.some((i) => i.kind === 'run' && i.run.outcome === 'failed'));
  });

  it('says Copied on a copied reply for a moment', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const { ask } = await chatting();
    ask.markCopied(3);
    assert.equal(ask.state.copied, 3);
    mock.timers.tick(C.COPIED_MS);
    assert.equal(ask.state.copied, null);
  });

  it('stops hearing the main process once disposed', async () => {
    const { fake, ask } = await chatting();
    ask.dispose();
    for (const event of ['onAgentEvent', 'onDevLog', 'onWsStatus', 'onSettingsChanged', 'onFingerprintChanged'])
      assert.equal(fake.listeners(event), 0, event);
  });
});
