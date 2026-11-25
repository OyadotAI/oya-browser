/**
 * Unit tests for the commands that read what a page logged and fetched: a lane reads its own
 * tab and nothing else, whatever it sends, because the rings hold every tab's entries.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { TAB_COMMANDS } from '../../../../src/main/connection/tab-commands.ts';
import { Observer } from '../../../../src/main/observe/observer.ts';

/** The person's tab and the agent's, each with an error logged and a request refused. */
function observed() {
  const observer = new Observer();
  observer.addConsole({ level: 3, message: 'theirs', webContentsId: 1 });
  observer.addConsole({ level: 3, message: 'mine', webContentsId: 2 });
  observer.addRequest({ url: 'https://bank.example.com/account', statusCode: 500, webContentsId: 1 });
  observer.addRequest({ url: 'http://localhost:3000/api', statusCode: 404, webContentsId: 2 });
  return observer;
}

/**
 * A lane's tab as the commands ask about it: the page it shows, whether agents are held to their
 * own tabs, its claim, and the tabs it opened.
 */
const lane = (view: unknown, scoped = true, id: number | null = 2, opened: number[] = [2]) => ({
  view: () => view,
  scoped: () => scoped,
  id,
  openedByThisLane: (tabId: number) => opened.includes(tabId),
});

/** A runner whose lane is `agentTab`, recording what each command answered. */
function runner(view: unknown, agentTab = lane(view), observer: Observer | undefined = observed()) {
  const sent: any[] = [];
  const sendResult = (...answer: unknown[]) => void sent.push(answer);
  return { sent, runner: { deps: { observer, agentTab, tabs: { activeTabId: 2 } }, sendResult } as any };
}

/** What a read answers when the lane has no tab to read. */
const NO_TAB = 'No tab of your own to read. Open one with open_tab first.';

/** The agent's own tab. */
const MINE = { webContents: { id: 2, isDestroyed: () => false } };

it("read_console returns only what the lane's own tab logged", () => {
  const { sent, runner: r } = runner(MINE);
  void TAB_COMMANDS.read_console(r, 'c1', {});
  assert.deepEqual(
    sent[0][2].entries.map((e: any) => e.message),
    ['mine'],
  );
});

it("read_network returns only what the lane's own tab fetched", () => {
  const { sent, runner: r } = runner(MINE);
  void TAB_COMMANDS.read_network(r, 'c1', { failedOnly: true });
  assert.deepEqual(
    sent[0][2].requests.map((e: any) => e.url),
    ['http://localhost:3000/api'],
  );
});

it("a read sent with no parameters at all still reads only the lane's own tab", () => {
  const { sent, runner: r } = runner(MINE);
  void TAB_COMMANDS.read_console(r, 'c1', undefined);
  void TAB_COMMANDS.read_network(r, 'c2', undefined);
  assert.equal(sent[0][2].entries.length, 1);
  assert.equal(sent[1][2].requests.length, 1);
});

it('a page id the caller sends is ignored: the lane cannot choose whose tab it reads', () => {
  const { sent, runner: r } = runner(MINE);
  void TAB_COMMANDS.read_console(r, 'c1', { webContentsId: 1 } as any);
  void TAB_COMMANDS.read_network(r, 'c2', { webContentsId: 1 } as any);
  assert.deepEqual(
    sent[0][2].entries.map((e: any) => e.message),
    ['mine'],
  );
  assert.deepEqual(
    sent[1][2].requests.map((e: any) => e.url),
    ['http://localhost:3000/api'],
  );
});

for (const command of ['read_console', 'read_network']) {
  it(`${command} fails when the lane has no tab, and reads nothing`, () => {
    const { sent, runner: r } = runner(null);
    void TAB_COMMANDS[command](r, 'c1', {});
    assert.deepEqual(sent[0], ['c1', false, null, NO_TAB]);
  });

  it(`${command} reads nothing for a lane that has not taken a tab yet, though the person's tab is shown`, () => {
    const shown = { webContents: { id: 1, isDestroyed: () => false } };
    const { sent, runner: r } = runner(shown, lane(shown, true, null));
    void TAB_COMMANDS[command](r, 'c1', {});
    assert.deepEqual(sent[0], ['c1', false, null, NO_TAB]);
  });

  it(`${command}, on the local port, reads the shown tab when this lane opened it`, () => {
    const { sent, runner: r } = runner(MINE, lane(MINE, false, null, [2]));
    void TAB_COMMANDS[command](r, 'c1', {});
    assert.equal(sent[0][1], true);
    assert.equal(Object.values(sent[0][2] as object)[0].length, 1);
  });

  it(`${command}, on the local port, reads nothing when the shown tab is the person's or another agent's`, () => {
    const { sent, runner: r } = runner(MINE, lane(MINE, false, null, []));
    void TAB_COMMANDS[command](r, 'c1', {});
    assert.deepEqual(sent[0], ['c1', false, null, NO_TAB]);
  });

  it(`${command} fails when the lane's tab is gone`, () => {
    const { sent, runner: r } = runner({ webContents: { id: 2, isDestroyed: () => true } });
    void TAB_COMMANDS[command](r, 'c1', {});
    assert.equal(sent[0][1], false);
  });

  it(`${command} answers an empty list when nothing is being observed`, () => {
    const sent: any[] = [];
    const r = {
      deps: { agentTab: lane(MINE), tabs: { activeTabId: 2 } },
      sendResult: (...a: unknown[]) => void sent.push(a),
    } as any;
    void TAB_COMMANDS[command](r, 'c1', {});
    assert.equal(sent[0][1], true);
    assert.equal(Object.values(sent[0][2] as object)[0].length, 0);
  });
}
