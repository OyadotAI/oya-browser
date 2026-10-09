/** Ephemeral native sessions cannot leak between owners, survive disconnect or expose failed setup. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeContexts, privateSession } from '../../../../src/main/native-contexts/index.ts';
/** Native session seam records real cleanup stages without opening a browser. */
function fixture(configure: () => Promise<void> = async () => {}, failures: readonly string[] = []) {
  const calls: string[] = [],
    partitions: string[] = [];
  const session = {
    closeAllConnections: async () => {
      record('connections');
    },
    clearStorageData: async () => {
      record('storage');
    },
    clearCache: async () => {
      record('cache');
    },
  } as any;
  const contexts = new NativeContexts({
    create: (partition) => {
      partitions.push(partition);
      return session;
    },
    configure,
    close: () => {
      record('tabs');
    },
  });
  /** Record cleanup ordering while allowing each native resource to fail independently. */
  function record(stage: string) {
    calls.push(stage);
    if (failures.includes(stage)) throw Error(stage + ' failed');
  }
  return { contexts, session, calls, partitions };
}
test('context ownership is private and tombstoned sessions never become public', async () => {
  const a = fixture(),
    b = fixture(),
    id = await a.contexts.create();
  assert.equal(a.partitions[0].startsWith('persist:'), false);
  assert.deepEqual(a.contexts.list(), [id]);
  assert.equal(a.contexts.get(id), a.session);
  assert.throws(() => b.contexts.get(id), /foreign/);
  assert.equal(b.contexts.visible(a.session), false);
  assert.equal(privateSession(a.session), true);
  await a.contexts.remove(id);
  assert.equal(a.contexts.visible(a.session), false);
  assert.equal(b.contexts.visible(a.session), false);
  assert.deepEqual(a.calls, ['tabs', 'connections', 'storage', 'cache']);
});
test('failed setup closes tabs and clears the native session before rejecting', async () => {
  const f = fixture(async () => {
    throw Error('policy failed');
  });
  await assert.rejects(f.contexts.create(), /policy failed/);
  assert.deepEqual(f.contexts.list(), []);
  assert.deepEqual(f.calls, ['tabs', 'connections', 'storage', 'cache']);
});
test('disconnect fences late setup and cleans newly installed resources a second time', async () => {
  let resume!: () => void;
  const f = fixture(
    () =>
      new Promise<void>((resolve) => {
        resume = resolve;
      }),
  );
  const created = f.contexts.create();
  const rejected = assert.rejects(created, /disconnected/);
  await f.contexts.dispose();
  resume();
  await rejected;
  assert.deepEqual(f.contexts.list(), []);
  assert.deepEqual(f.calls, ['tabs', 'connections', 'storage', 'cache', 'tabs', 'connections', 'storage', 'cache']);
  await assert.rejects(f.contexts.create(), /closed/);
});

/** Hold policy setup at a real asynchronous boundary without wall-clock sleeps. */
function deferred() {
  let resolve!: () => void, reject!: (error: Error) => void;
  const promise = new Promise<void>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
/** Only the test-owned factory knows an unpublished context's opaque id. */
function pendingId(f: ReturnType<typeof fixture>): string {
  return f.partitions[0].slice('oya-native-context-'.length);
}

test('pending policy reserves ownership but exposes no identity, session or private tab', async () => {
  const setup = deferred(),
    f = fixture(() => setup.promise),
    other = fixture();
  const created = f.contexts.create(),
    id = pendingId(f);
  assert.deepEqual(f.contexts.list(), []);
  assert.throws(() => f.contexts.get(id), /Unknown or foreign/);
  assert.equal(f.contexts.id(f.session), undefined);
  assert.equal(f.contexts.visible(f.session), false);
  assert.equal(other.contexts.visible(f.session), false);
  assert.equal(privateSession(f.session), true);
  setup.resolve();
  assert.equal(await created, id);
  assert.deepEqual(f.contexts.list(), [id]);
  assert.equal(f.contexts.id(f.session), id);
  assert.equal(f.contexts.get(id), f.session);
  assert.equal(f.contexts.visible(f.session), true);
  await f.contexts.dispose();
});

test('cancelling pending setup cannot republish an id when configuration completes late', async () => {
  const setup = deferred(),
    f = fixture(() => setup.promise);
  const rejected = assert.rejects(f.contexts.create(), /context cancelled/);
  const id = pendingId(f);
  await f.contexts.remove(id);
  setup.resolve();
  await rejected;
  assert.deepEqual(f.contexts.list(), []);
  assert.throws(() => f.contexts.get(id), /Unknown or foreign/);
  assert.equal(f.contexts.id(f.session), undefined);
  assert.equal(f.contexts.visible(f.session), false);
  assert.deepEqual(f.calls, ['tabs', 'connections', 'storage', 'cache', 'tabs', 'connections', 'storage', 'cache']);
});

test('a disconnected owner cannot expose default-profile or retired private sessions', async () => {
  const f = fixture(),
    defaultSession = {} as any;
  await f.contexts.create();
  assert.equal(f.contexts.visible(defaultSession), true);
  await f.contexts.dispose();
  assert.equal(f.contexts.visible(defaultSession), false);
  assert.equal(f.contexts.id(f.session), undefined);
  assert.equal(f.contexts.visible(f.session), false);
});

test('pending setup counts against the context quota before any id is published', async () => {
  const setup = deferred(),
    sessions: any[] = [];
  const contexts = new NativeContexts({
    create: () => {
      const session = {
        closeAllConnections: async () => {},
        clearStorageData: async () => {},
        clearCache: async () => {},
      } as any;
      sessions.push(session);
      return session;
    },
    configure: () => setup.promise,
    close: () => {},
  });
  const attempts = Array.from({ length: 8 }, () => contexts.create());
  const rejections = attempts.map((attempt) => assert.rejects(attempt, /disconnected/));
  assert.deepEqual(contexts.list(), []);
  await assert.rejects(contexts.create(), /limit/);
  assert.equal(sessions.length, 8);
  await contexts.dispose();
  setup.resolve();
  await Promise.all(rejections);
  assert.ok(sessions.every((session) => !contexts.visible(session) && privateSession(session)));
});

for (const stage of ['tabs', 'connections', 'storage', 'cache'])
  test('cleanup failure in ' + stage + ' still attempts every later resource', async () => {
    const f = fixture(undefined, [stage]),
      id = await f.contexts.create();
    await assert.rejects(f.contexts.remove(id), (error: any) => {
      assert.ok(error instanceof AggregateError);
      assert.deepEqual(
        error.errors.map((failure: Error) => failure.message),
        [stage + ' failed'],
      );
      return true;
    });
    assert.deepEqual(f.calls, ['tabs', 'connections', 'storage', 'cache']);
    assert.deepEqual(f.contexts.list(), []);
    assert.throws(() => f.contexts.get(id), /foreign/);
    assert.equal(f.contexts.id(f.session), undefined);
    assert.equal(f.contexts.visible(f.session), false);
    assert.equal(privateSession(f.session), true);
  });

test('all cleanup failures remain available without restoring revoked access', async () => {
  const stages = ['tabs', 'connections', 'storage', 'cache'],
    f = fixture(undefined, stages);
  const id = await f.contexts.create();
  await assert.rejects(f.contexts.remove(id), (error: any) => {
    assert.deepEqual(
      error.errors.map((failure: Error) => failure.message),
      stages.map((stage) => stage + ' failed'),
    );
    return true;
  });
  assert.deepEqual(f.calls, stages);
  assert.equal(f.contexts.visible(f.session), false);
});

test('setup and cleanup failures are both retained while the private session stays hidden', async () => {
  const policyError = Error('policy failed'),
    f = fixture(async () => {
      throw policyError;
    }, ['tabs', 'storage']);
  await assert.rejects(f.contexts.create(), (error: any) => {
    assert.ok(error instanceof AggregateError);
    assert.equal(error.errors[0], policyError);
    assert.deepEqual(
      error.errors[1].errors.map((failure: Error) => failure.message),
      ['tabs failed', 'storage failed'],
    );
    return true;
  });
  assert.deepEqual(f.calls, ['tabs', 'connections', 'storage', 'cache']);
  assert.equal(f.contexts.visible(f.session), false);
  assert.deepEqual(f.contexts.list(), []);
});

test('revocation is visible before slow native storage cleanup completes', async () => {
  const f = fixture(),
    entered = deferred(),
    resume = deferred(),
    id = await f.contexts.create();
  f.session.clearStorageData = async () => {
    f.calls.push('storage');
    entered.resolve();
    await resume.promise;
  };
  const removed = f.contexts.remove(id);
  await entered.promise;
  assert.deepEqual(f.contexts.list(), []);
  assert.throws(() => f.contexts.get(id), /foreign/);
  assert.equal(f.contexts.id(f.session), undefined);
  assert.equal(f.contexts.visible(f.session), false);
  resume.resolve();
  await removed;
  assert.deepEqual(f.calls, ['tabs', 'connections', 'storage', 'cache']);
});

test('disconnect retires every context even when one native cleanup fails', async () => {
  const calls: string[][] = [],
    sessions: any[] = [];
  const contexts = new NativeContexts({
    create: () => {
      const trace: string[] = [];
      calls.push(trace);
      const session = {
        closeAllConnections: async () => {
          trace.push('connections');
          if (session === sessions[0]) throw Error('offline');
        },
        clearStorageData: async () => {
          trace.push('storage');
        },
        clearCache: async () => {
          trace.push('cache');
        },
      } as any;
      sessions.push(session);
      return session;
    },
    configure: async () => {},
    close: (session) => {
      assert.deepEqual(contexts.list(), [], 'disconnect hides every id before invoking any close callback');
      calls[sessions.indexOf(session)].push('tabs');
    },
  });
  const ids = await Promise.all([contexts.create(), contexts.create()]);
  await assert.rejects(contexts.dispose(), /cleanup failed/);
  assert.deepEqual(
    calls,
    Array.from({ length: 2 }, () => ['tabs', 'connections', 'storage', 'cache']),
  );
  assert.deepEqual(contexts.list(), []);
  for (const id of ids) assert.throws(() => contexts.get(id), /foreign/);
  assert.ok(sessions.every((session) => !contexts.visible(session)));
});
