/** Native context retirement must attempt network, downloads, each owned tab and native storage independently. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { appNativeContexts } from '../../../../src/main/app/native-contexts.ts';

/** Minimal native-only application seams with exact-session tabs and independently failing resources. */
function setup(failure: string) {
  const calls: string[] = [];
  /** Record native cleanup attempts without hiding a later resource behind an earlier failure. */
  function step(name: string) {
    calls.push(name);
    if (name === failure) throw Error(name + ' failed');
  }
  const session: any = {
    setPermissionRequestHandler() {},
    setPermissionCheckHandler() {},
    setProxy: async () => {},
    closeAllConnections: async () => step('connections'),
    clearStorageData: async () => step('storage'),
    clearCache: async () => step('cache'),
  };
  const tabs = [1, 2].map((id) => ({ id, view: { webContents: { session } } }));
  tabs.push({ id: 3, view: { webContents: { session: {} } } });
  const deps: any = {
    nativeBrowsing: true,
    persona: { active: null },
    governance: null,
    electron: { app: {}, session: { fromPartition: () => session } },
    windows: {
      allTabs: () => tabs,
      owner: (id: number) => ({
        tabs: {
          closeTab: (actual: number, options: unknown) => {
            assert.equal(actual, id);
            assert.deepEqual(options, { keepOne: false });
            step('tab' + id);
          },
        },
      }),
    },
  };
  const resources: any = {
    network: { install() {}, remove: () => step('network') },
    downloads: { install() {}, remove: () => step('downloads') },
  };
  return { contexts: appNativeContexts(deps, resources), calls, session };
}

for (const failure of ['', 'network', 'downloads', 'tab1', 'tab2'])
  test('retirement preserves other resources after ' + (failure || 'successful cleanup'), async () => {
    const f = setup(failure),
      id = await f.contexts.create();
    if (failure) await assert.rejects(f.contexts.remove(id), /retirement failed/);
    else await f.contexts.remove(id);
    assert.deepEqual(f.calls, ['network', 'downloads', 'tab1', 'tab2', 'connections', 'storage', 'cache']);
    assert.deepEqual(f.contexts.list(), []);
    assert.equal(f.contexts.visible(f.session), false);
  });
