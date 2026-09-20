/**
 * Unit tests for the page scripts that dump and replay a profile's origin
 * storage, run in a VM against fake localStorage and sessionStorage.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { runInNewContext } from 'node:vm';
import { ORIGIN_STORAGE_JS, restoreStorageJS } from '../../../../src/modules/gateway/profile-storage-js.ts';

/** A Web Storage stand-in over a plain object. */
function storage(items: Record<string, string> = {}) {
  const data = { ...items };
  return {
    data,
    get length() {
      return Object.keys(data).length;
    },
    key: (i: number) => Object.keys(data)[i],
    getItem: (k: string) => data[k],
    setItem: (k: string, v: string) => void (data[k] = String(v)),
  };
}

describe('ORIGIN_STORAGE_JS', () => {
  it("dumps the origin's localStorage and sessionStorage", () => {
    const result = runInNewContext(ORIGIN_STORAGE_JS, {
      location: { origin: 'https://a.com' },
      localStorage: storage({ token: 't' }),
      sessionStorage: storage({ step: '2' }),
    });
    assert.deepEqual(JSON.parse(JSON.stringify(result)), {
      origin: 'https://a.com',
      local: { token: 't' },
      session: { step: '2' },
    });
  });

  it('is null where storage cannot be read', () => {
    const context = { location: { origin: 'null' } };
    Object.defineProperty(context, 'localStorage', {
      get() {
        throw new Error('SecurityError');
      },
    });
    assert.equal(runInNewContext(ORIGIN_STORAGE_JS, context), null);
  });
});

describe('restoreStorageJS', () => {
  it('writes saved entries back into both stores', () => {
    const localStorage = storage();
    const sessionStorage = storage();
    const ok = runInNewContext(restoreStorageJS({ local: { a: '1' }, session: { b: '2' } }), {
      localStorage,
      sessionStorage,
    });
    assert.equal(ok, true);
    assert.deepEqual([localStorage.data, sessionStorage.data], [{ a: '1' }, { b: '2' }]);
  });

  it('writes nothing for a store that was not saved', () => {
    const localStorage = storage();
    runInNewContext(restoreStorageJS({}), { localStorage, sessionStorage: storage() });
    assert.deepEqual(localStorage.data, {});
  });

  it('embeds saved values as data, never as code', () => {
    const localStorage = storage();
    const evil = { local: { k: '"); throw new Error("pwned' } };
    runInNewContext(restoreStorageJS(evil), { localStorage, sessionStorage: storage() });
    assert.equal(localStorage.data.k, '"); throw new Error("pwned');
  });

  it('is false where storage cannot be written', () => {
    const blocked = {
      setItem: () => {
        throw new Error('QuotaExceeded');
      },
    };
    assert.equal(
      runInNewContext(restoreStorageJS({ local: { a: '1' } }), { localStorage: blocked, sessionStorage: blocked }),
      false,
    );
  });
});
