/**
 * Unit tests for src/main/app/notification-permissions.ts: a site asks once from the tab in
 * front, Allow and Block are both remembered per profile, a remembered Allow holds in a
 * background tab, and agent, governed and private tabs never get notifications.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { fixture as mediaFixture } from '../../support/media-fixture.ts';
import { NotificationPermissions } from '../../../../src/main/app/notification-permissions.ts';

const SITE = 'https://mail.test';
const PAGE = `${SITE}/inbox`;

/** The notification service over the media fixture, with a tab list and a switch for "another tab is in front". */
function fixture() {
  const f = mediaFixture();
  f.state.url = PAGE;
  const state = Object.assign(f.state, { background: false, privateSession: false });
  const other = { webContents: {} };
  f.deps.tabs = {
    activeTabId: 1,
    list: [{ id: 1, view: { webContents: f.contents } }],
    getActiveView: () => (state.background ? other : { webContents: f.contents }),
    get privateSession() {
      return state.privateSession;
    },
  } as any;
  f.deps.agentTab.humanMay = (id: number) => id === 1 && f.state.interactive;
  const deps = { ...f.deps, mediaPermissions: f.service };
  const service = new NotificationPermissions(deps as any);
  const request = () => service.request(f.contents as any, PAGE);
  const check = () => service.check(f.contents as any, `${SITE}/`);
  return { ...f, state, service, request, check };
}

it('a first ask from the tab in front shows the Block-first box, and Allow is remembered', async () => {
  const f = fixture();
  assert.equal(f.check(), false);
  assert.equal(await f.request(), true);
  assert.equal(f.prompts.length, 1);
  assert.equal(f.prompts[0].message, 'Show notifications?');
  assert.deepEqual([f.prompts[0].defaultId, f.prompts[0].cancelId], [0, 0]);
  assert.equal(f.check(), true);
  assert.equal(await f.request(), true, 'shown again without a box, as Electron asks before each notification');
  assert.equal(f.prompts.length, 1);
});

it('a remembered Allow holds in a background tab, without asking', async () => {
  const f = fixture();
  await f.request();
  f.state.background = true;
  assert.equal(f.check(), true);
  assert.equal(await f.request(), true);
  assert.equal(f.prompts.length, 1);
});

it('a background tab that never asked is refused without a box', async () => {
  const f = fixture();
  f.state.background = true;
  assert.equal(await f.request(), false);
  assert.deepEqual(f.prompts, []);
});

it('Block is remembered, so the site is refused without a box after a reload', async () => {
  const f = fixture();
  f.state.response = 0;
  assert.equal(await f.request(), false);
  assert.equal(await f.request(), false);
  assert.equal(f.prompts.length, 1);
  assert.equal(f.check(), false);
});

it("forgetting the profile's site permissions makes the site ask again", async () => {
  const f = fixture();
  f.state.response = 0;
  await f.request();
  assert.deepEqual(f.deps.sitePermissions.origins('persist:one'), [SITE]);
  f.deps.sitePermissions.forget('persist:one');
  f.state.response = 1;
  assert.equal(await f.request(), true);
  assert.equal(f.prompts.length, 2);
});

it('a tab an agent drives is refused, even with a remembered Allow', async () => {
  const f = fixture();
  await f.request();
  f.state.interactive = false;
  assert.equal(f.check(), false);
  assert.equal(await f.request(), false);
});

it('a governed browser and a private window are refused without a box', async () => {
  for (const restrict of [(f) => (f.deps.governance.configuration = {}), (f) => (f.state.privateSession = true)]) {
    const f = fixture();
    restrict(f);
    assert.equal(await f.request(), false);
    assert.equal(f.check(), false);
    assert.deepEqual(f.prompts, []);
  }
});

it('another profile does not share the answer', async () => {
  const f = fixture();
  await f.request();
  f.deps.persona.partitionName = () => 'persist:two';
  assert.equal(f.check(), false);
});

it('an insecure page, or a request for another site, is refused without a box', async () => {
  const f = fixture();
  assert.equal(await f.service.request(f.contents as any, 'https://other.test/'), false);
  f.state.url = 'http://mail.test/inbox';
  assert.equal(await f.service.request(f.contents as any, 'http://mail.test/inbox'), false);
  assert.deepEqual(f.prompts, []);
});

it('a navigation while the box is open stores nothing', async () => {
  const f = fixture();
  f.deps.electron.dialog.showMessageBox = async () => {
    f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false });
    return { response: 1 };
  };
  assert.equal(await f.request(), false);
  assert.deepEqual(f.deps.sitePermissions.origins('persist:one'), []);
});

it('a burst of requests shows one box', async () => {
  const f = fixture();
  const answers = await Promise.all([f.request(), f.request(), f.request()]);
  assert.deepEqual(answers, [true, true, true]);
  assert.equal(f.prompts.length, 1);
});

it('a service worker may notify for a remembered site while the person has a tab open on it', async () => {
  const f = fixture();
  assert.equal(f.service.check(null, `${SITE}/`), false, 'nothing remembered yet');
  await f.request();
  f.state.background = true;
  assert.equal(f.service.check(null, `${SITE}/`), true);
  assert.equal(await f.service.request(null, `${SITE}/`), true);
  assert.equal(f.prompts.length, 1);
});

it('a service worker is refused when no tab of the person is open on its site', async () => {
  const f = fixture();
  await f.request();
  assert.equal(f.service.check(null, 'https://other.test/'), false, 'no tab on that site');
  f.state.interactive = false;
  assert.equal(f.service.check(null, `${SITE}/`), false, "the only tab is an agent's");
  f.state.interactive = true;
  f.state.privateSession = true;
  assert.equal(f.service.check(null, `${SITE}/`), false, 'private windows never count');
});

it("a popup window opened by a tab an agent drives is the agent's, even with a remembered Allow", async () => {
  const f = fixture();
  await f.request();
  Object.assign(f.deps.tabs.list[0], { window: {}, openerId: 2 });
  assert.equal(f.check(), false);
  f.deps.agentTab.humanMay = () => f.state.interactive;
  assert.equal(f.check(), true, "a popup the person's tab opened is the person's");
});
