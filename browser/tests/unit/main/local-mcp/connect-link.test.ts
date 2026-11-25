/**
 * Unit tests for connecting an agent from an oya://agent-connect link: the link can only ask,
 * only a person's click connects, and nothing the link carries reaches the agent's config.
 */
import { it, mock, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { AgentConnectLink, isAgentConnectLink } from '../../../../src/main/local-mcp/connect-link.ts';
import {
  CONNECT_LINK_CANCEL_QUIET_MS,
  CONNECT_LINK_COOLDOWN_MS,
  CONNECT_LINK_PORT_WAIT_MS,
} from '../../../../src/main/local-mcp/constants.ts';

/** This app's own endpoint, the only one that may ever be written. */
const OWN = { url: 'http://127.0.0.1:47321/mcp', token: 'oya_local_own' };
const LINK = 'oya://agent-connect?agent=claude-code';

/** A flow over one fake agent, recording each dialog shown and each endpoint connected. */
function flow(over: Record<string, unknown> = {}) {
  const asked: any[] = [];
  const connectedWith: unknown[] = [];
  const state = { answer: 1, connected: false, endpoint: OWN as typeof OWN | null, shown: 0, fail: '' };
  const agent = {
    name: 'Claude Code',
    installed: () => true,
    connected: () => state.connected,
    connect: async (endpoint: unknown) => {
      if (state.fail) throw Error(state.fail);
      connectedWith.push(endpoint);
    },
    disconnect: async () => {},
  };
  const link = new AgentConnectLink({
    ask: async (options) => (asked.push(options), { response: state.answer, checkboxChecked: false }),
    show: () => void state.shown++,
    endpoint: () => state.endpoint,
    installed: () => true,
    agents: { 'claude-code': agent } as any,
    ...over,
  });
  return { link, asked, connectedWith, state };
}

beforeEach(() => mock.timers.enable({ apis: ['setTimeout', 'Date'] }));
afterEach(() => mock.timers.reset());

it('knows a connect link from a pairing link or a web address', () => {
  assert.equal(isAgentConnectLink(LINK), true);
  for (const other of ['oya://connect?code=1&server=wss://a.test/ws', 'https://agent-connect/', 'not a link', ''])
    assert.equal(isAgentConnectLink(other), false, other);
});

it("connects with this app's own endpoint once the person clicks Connect", async () => {
  const { link, asked, connectedWith, state } = flow();
  await link.open(LINK);
  assert.equal(state.shown, 1, 'the window is brought forward first');
  assert.equal(asked[0].message, 'Connect Claude Code to Oya Browser?');
  assert.deepEqual(connectedWith, [OWN]);
  assert.equal(asked[1].message, 'Claude Code is connected');
});

it('makes Cancel the default, so Return and Escape never connect', async () => {
  const { link, asked } = flow();
  await link.open(LINK);
  assert.deepEqual(asked[0].buttons, ['Cancel', 'Connect Claude Code']);
  assert.equal(asked[0].defaultId, 0);
  assert.equal(asked[0].cancelId, 0);
});

it('changes nothing and says nothing more when the person cancels', async () => {
  const { link, asked, connectedWith, state } = flow();
  state.answer = 0;
  await link.open(LINK);
  assert.equal(asked.length, 1);
  assert.deepEqual(connectedWith, []);
});

it('ignores a token, an address or anything else the link carries', async () => {
  const { link, connectedWith } = flow();
  await link.open(`${LINK}&token=evil&url=https://evil.test/mcp&server=wss://evil.test`);
  assert.deepEqual(connectedWith, [OWN]);
});

it("refuses an agent it does not know without repeating the link's text", async () => {
  const { link, asked, connectedWith } = flow();
  await link.open('oya://agent-connect?agent=<script>evil</script>');
  assert.equal(asked.length, 1);
  assert.equal(asked[0].message, 'This link names an agent Oya does not know');
  assert.ok(!JSON.stringify(asked[0]).includes('evil'));
  assert.deepEqual(connectedWith, []);
});

it('refuses a name that is a property of every object, not an agent', async () => {
  for (const name of ['constructor', '__proto__', 'toString', '']) {
    const { link, asked, connectedWith } = flow();
    await link.open(`oya://agent-connect?agent=${name}`);
    assert.equal(asked[0].message, 'This link names an agent Oya does not know', name);
    assert.deepEqual(connectedWith, []);
  }
});

it('says the agent is not installed, and asks for no consent', async () => {
  const { link, asked, connectedWith } = flow({ installed: () => false });
  await link.open(LINK);
  assert.equal(asked.length, 1);
  assert.equal(asked[0].message, 'Claude Code is not installed on this computer');
  assert.deepEqual(connectedWith, []);
});

it('says an agent is already connected, and asks for no consent', async () => {
  const { link, asked, connectedWith, state } = flow();
  state.connected = true;
  await link.open(LINK);
  assert.equal(asked.length, 1);
  assert.equal(asked[0].message, 'Claude Code is already connected');
  assert.deepEqual(connectedWith, []);
});

it('says why when connecting fails, and points at the footer', async () => {
  const { link, asked, state } = flow();
  state.fail = 'The config file could not be written';
  await link.open(LINK);
  assert.equal(asked[1].message, 'Could not connect Claude Code');
  assert.match(asked[1].detail, /could not be written\. Nothing else was changed\. Try Connect next to Claude Code/);
});

it('waits for the port when the link launched the app, then asks', async () => {
  const { link, asked, state } = flow();
  state.endpoint = null;
  const opened = link.open(LINK);
  await Promise.resolve();
  assert.equal(asked.length, 0, 'nothing is asked while the port is not up');
  state.endpoint = OWN;
  mock.timers.tick(CONNECT_LINK_PORT_WAIT_MS);
  await opened;
  assert.equal(asked[0].message, 'Connect Claude Code to Oya Browser?');
});

it('gives up, and says so, when the port never comes up', async () => {
  const { link, asked, connectedWith, state } = flow();
  state.endpoint = null;
  const opened = link.open(LINK);
  for (let waited = 0; waited <= CONNECT_LINK_PORT_WAIT_MS; waited += CONNECT_LINK_PORT_WAIT_MS / 100) {
    mock.timers.tick(CONNECT_LINK_PORT_WAIT_MS / 100);
    await new Promise((resolve) => setImmediate(resolve));
  }
  await opened;
  assert.equal(asked[0].message, 'Could not connect Claude Code');
  assert.deepEqual(connectedWith, []);
});

it('drops a second link while the first is still in front of the person', async () => {
  let release = () => {};
  const asked: unknown[] = [];
  const { link } = flow({
    ask: (options: unknown) => (
      asked.push(options),
      new Promise((resolve) => (release = () => resolve({ response: 0 })))
    ),
  });
  const first = link.open(LINK);
  await new Promise((resolve) => setImmediate(resolve));
  await link.open(LINK);
  assert.equal(asked.length, 1);
  release();
  await first;
});

it('drops links that follow at once, and answers again after the pause', async () => {
  const { link, asked } = flow({ installed: () => false });
  await link.open(LINK);
  await link.open(LINK);
  assert.equal(asked.length, 1, 'a link in a loop cannot stack dialogs');
  mock.timers.tick(CONNECT_LINK_COOLDOWN_MS);
  await link.open(LINK);
  assert.equal(asked.length, 2);
});

it('stays quiet much longer after the person cancels, so a page cannot keep asking', async () => {
  const { link, asked, state } = flow();
  state.answer = 0;
  await link.open(LINK);
  mock.timers.tick(CONNECT_LINK_COOLDOWN_MS);
  await link.open(LINK);
  assert.equal(asked.length, 1, 'the short pause is not enough after a Cancel');
  mock.timers.tick(CONNECT_LINK_CANCEL_QUIET_MS);
  await link.open(LINK);
  assert.equal(asked.length, 2);
});
