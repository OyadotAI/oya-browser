/** An old or falsely advertised native engine must not pass the release-world probe. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { inspectWorld } from '../../../build/native-world-probe.cjs';

/** Fake only the engine seam; verify requests keep the exact native world identity. */
function fixture() {
  const calls: any[] = [];
  const replies = {
    context: { context: 'main' },
    isolatedContext: { context: 'private' },
    evaluate: null,
    close: {},
  };
  const frame = {
    async _runOyaRuntime(owner, context, operation, params) {
      calls.push({ owner, context, operation, params });
      return (
        replies[operation] ?? { result: { value: context === 'private' ? 'undefined:undefined:object' : 'undefined' } }
      );
    },
  };
  return { frame, replies, calls };
}

test('requires separate native identities and closes both owned runtimes', async () => {
  const { frame, calls } = fixture();
  await inspectWorld(frame);
  assert.deepEqual(
    calls.filter((c) => c.operation === 'close').map((c) => c.context),
    ['private', 'main'],
  );
  const privateCall = calls.find((c) => c.operation === 'evaluate' && c.context === 'private');
  assert.equal(privateCall.params.world, 'isolated-packaging-probe');
});

test('rejects an engine without isolated-world support', async () => {
  const { frame, replies } = fixture();
  replies.isolatedContext = { error: 'Unsupported operation' } as any;
  await assert.rejects(inspectWorld(frame), /probe failed/);
});

test('rejects a main-world fallback presented as an isolated world', async () => {
  const { frame, replies } = fixture();
  replies.isolatedContext.context = 'main';
  await assert.rejects(inspectWorld(frame), /not isolated/);
});

test('rejects shared globals and still closes native allocations', async () => {
  const { frame, replies, calls } = fixture();
  replies.evaluate = { result: { value: 'number' } } as any;
  await assert.rejects(inspectWorld(frame), /isolation probe failed/);
  assert.equal(calls.filter((c) => c.operation === 'close').length, 2);
});
