/** Explicit recording screenshots retain target identity and reject control changes during native capture. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { captureSpecifiedTab } from '../../../../src/main/connection/native-screenshot.ts';
/** A native image seam with no protocol transport. */
function fixture() {
  const image = { isEmpty: () => false, toJPEG: () => Buffer.from('jpeg'), toPNG: () => Buffer.from('png') };
  const tab: any = {
    id: 7,
    home: false,
    protection: 'protected',
    view: {
      webContents: { isDestroyed: () => false, getURL: () => 'https://example.test', capturePage: async () => image },
    },
  };
  const state = { mode: 'agent', mine: false };
  const deps: any = { control: { snapshot: () => state }, tabs: { list: [tab] } };
  return { tab, state, deps, image };
}
test('captures only the explicitly named protected native tab', async () => {
  const { deps } = fixture();
  assert.equal(await captureSpecifiedTab(deps, 7, 'jpeg'), 'data:image/jpeg;base64,anBlZw==');
  await assert.rejects(captureSpecifiedTab(deps, 8), /unavailable/);
  await assert.rejects(captureSpecifiedTab(deps, '7'), /Invalid/);
});
test('internal and unprotected pages never reach native capture', async () => {
  const { tab, deps } = fixture();
  for (const protection of ['pending', 'failed']) {
    tab.protection = protection;
    await assert.rejects(captureSpecifiedTab(deps, 7), /unprotected/);
  }
  tab.protection = 'protected';
  tab.home = true;
  await assert.rejects(captureSpecifiedTab(deps, 7), /unavailable/);
});
test('human takeover during capture discards captured pixels', async () => {
  const { deps, tab, state, image } = fixture();
  tab.view.webContents.capturePage = async () => {
    state.mode = 'human';
    state.mine = true;
    return image;
  };
  await assert.rejects(captureSpecifiedTab(deps, 7), /agent control/);
});

test('takeover and return during capture discard pixels even when agent mode is restored', async () => {
  const { deps, tab, state, image } = fixture();
  Object.assign(state, { revision: 1 });
  tab.view.webContents.capturePage = async () => {
    Object.assign(state, { mode: 'human', mine: true, revision: 2 });
    await Promise.resolve();
    Object.assign(state, { mode: 'agent', mine: false, revision: 3 });
    return image;
  };
  await assert.rejects(captureSpecifiedTab(deps, 7), /owner changed/);
});

test('explicit-tab capture waits for original text restoration', async () => {
  const { deps, tab, image } = fixture();
  let release!: () => void;
  let captured = false;
  deps.translation = {
    released: async (view: unknown) => {
      assert.equal(view, tab.view);
      await new Promise<void>((resolve) => {
        release = resolve;
      });
    },
  };
  tab.view.webContents.capturePage = async () => {
    captured = true;
    return image;
  };
  const capture = captureSpecifiedTab(deps, 7);
  await new Promise(setImmediate);
  assert.equal(captured, false);
  release();
  await capture;
  assert.equal(captured, true);
});

test('a live restoration failure prevents explicit-tab capture', async () => {
  const { deps, tab } = fixture();
  let captured = false;
  deps.translation = {
    released: async () => {
      throw Error('original page unavailable');
    },
  };
  tab.view.webContents.capturePage = async () => {
    captured = true;
    throw Error('must not capture');
  };
  await assert.rejects(captureSpecifiedTab(deps, 7), /original page unavailable/);
  assert.equal(captured, false);
});
