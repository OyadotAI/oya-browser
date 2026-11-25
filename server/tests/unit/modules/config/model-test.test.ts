/** Model diagnostics require an own key, enforce normal admission and count one bounded provider attempt. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { testModel, type ModelTestDeps } from '../../../../src/modules/config/model-test.ts';
import { LlmError } from '../../../../src/platform/llm/index.ts';
import { MODEL_TEST_MAX_TOKENS } from '../../../../src/modules/config/constants.ts';

/** Every provider and policy seam is synthetic; no configuration or network is touched. */
function fixture() {
  const value = {
    provider: 'gemini',
    own: true,
    apiKey: 'synthetic-provider-key',
    baseUrl: 'https://provider.test/v1',
    model: 'catalog-model',
  };
  const sent: any[] = [],
    billed: any[] = [];
  const deps: ModelTestDeps = {
    snapshot: () => ({ ...value }),
    admit: async () => ({ llm: { hosted: false }, budget: async () => {} }) as any,
    send: async (request) => {
      sent.push(request);
      return { choices: [{ message: { content: 'OK' } }], usage: { prompt_tokens: 3, completion_tokens: 1 } };
    },
    bill: ((context, usage) => billed.push({ context, usage })) as any,
  };
  return { value, sent, billed, deps };
}
it('a successful test sends one tool-free capped request and counts its actual usage', async () => {
  const f = fixture(),
    abort = new AbortController();
  assert.deepEqual(await testModel('workspace', 'gemini', abort.signal, f.deps), {
    ok: true,
    provider: 'gemini',
    model: 'catalog-model',
  });
  assert.equal(f.sent.length, 1);
  assert.equal(f.sent[0].maxTokens, MODEL_TEST_MAX_TOKENS);
  assert.equal(f.sent[0].tools, undefined);
  assert.deepEqual(f.sent[0].messages, [{ role: 'user', content: 'Reply with OK only.' }]);
  assert.equal(f.billed.length, 1);
  assert.equal(f.billed[0].usage.completion_tokens, 1);
  abort.abort();
  assert.equal(f.sent[0].signal.aborted, true);
});
it('host fallback or a different selected provider never spends a model credential', async () => {
  const f = fixture();
  f.value.own = false;
  await assert.rejects(
    testModel('workspace', 'gemini', undefined, f.deps),
    (error: any) => error.code === 'model_key_missing',
  );
  f.value.own = true;
  await assert.rejects(
    testModel('workspace', 'anthropic', undefined, f.deps),
    (error: any) => error.code === 'model_changed',
  );
  assert.equal(f.sent.length, 0);
  assert.equal(f.billed.length, 0);
});
it('normal model policy refusal prevents the test request', async () => {
  const f = fixture();
  f.deps.admit = async () => {
    throw Error('Project policy refused');
  };
  await assert.rejects(testModel('workspace', 'gemini', undefined, f.deps), /Project policy refused/);
  assert.equal(f.sent.length, 0);
});
it('configuration changed during the request is billed but never declared ready', async () => {
  const f = fixture(),
    send = f.deps.send;
  f.deps.send = async (request) => {
    const reply = await send(request);
    f.value.apiKey = 'new-synthetic-key';
    return reply;
  };
  await assert.rejects(
    testModel('workspace', 'gemini', undefined, f.deps),
    (error: any) => error.code === 'model_changed',
  );
  assert.equal(f.billed.length, 1);
});
it('refused keys, quota and transport failures expose safe recovery codes without vendor bodies', async () => {
  for (const [status, code] of [
    [400, 'model_key_refused'],
    [401, 'model_key_refused'],
    [429, 'model_quota'],
    [503, 'model_unavailable'],
  ] as const) {
    const f = fixture();
    let calls = 0;
    f.deps.send = async () => {
      calls++;
      throw new LlmError(status, 'sensitive vendor body');
    };
    await assert.rejects(
      testModel('workspace', 'gemini', undefined, f.deps),
      (error: any) => error.code === code && !error.message.includes('sensitive'),
    );
    assert.equal(calls, 1);
    assert.equal(f.billed.length, 0);
  }
});
it('caller cancellation reaches the provider and no empty completion can pass', async () => {
  const f = fixture(),
    abort = new AbortController(),
    begun = Promise.withResolvers<void>();
  f.deps.send = (request) =>
    new Promise((_resolve, reject) => {
      request.signal!.addEventListener('abort', () => reject(Error('aborted')), { once: true });
      begun.resolve();
    });
  const pending = testModel('workspace', 'gemini', abort.signal, f.deps);
  await begun.promise;
  abort.abort();
  await assert.rejects(pending, (error: any) => error.code === 'model_timeout');
  f.deps.send = async () => ({ choices: [] });
  await assert.rejects(
    testModel('workspace', 'gemini', undefined, f.deps),
    (error: any) => error.code === 'model_empty',
  );
});
