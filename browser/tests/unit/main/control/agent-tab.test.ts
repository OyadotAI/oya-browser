/**
 * Unit tests for per-tab agent control: the server's agent drives one tab, the
 * person keeps every other; nothing is fenced until the agent starts on a tab, and
 * local automation or another operator still takes the whole browser.
 */
import { it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { AgentTab, inLane } from '../../../../src/main/control/agent-tab.ts';
import { LANE_IDLE_MS } from '../../../../src/main/control/constants.ts';

/** The server's agent driving a connected desktop. */
const AGENT = { mode: 'agent', mine: false, interactive: false, connected: true, localClients: 0 };

/** An agent tab over tabs 1 and 2, the window showing `shown`, under control `state`. */
function setup(state: object = AGENT, shown = 1) {
  const views = { 1: { id: 'view-1' }, 2: { id: 'view-2' } };
  const tabs = {
    activeTabId: shown as number | null,
    created: [] as string[],
    closed: [] as number[],
    activateTab: (id: number) => void (tabs.activeTabId = id),
    closeTab: (id: number) => void tabs.closed.push(id),
    find: (id: number) => (id in views ? { id, view: views[id as keyof typeof views] } : undefined),
    getActiveView: () => views[tabs.activeTabId as 1 | 2],
    createTab: (url: string, activate: boolean) => {
      const id = 3 + tabs.created.length;
      tabs.created.push(`${url}${activate ? ' shown' : ''}`);
      (views as Record<number, object>)[id] = { id: `view-${id}` };
      return id;
    },
  };
  const control = { state, snapshot: () => ({ ...control.state }) };
  let changes = 0;
  const agentTab = new AgentTab({ control, tabs } as any, () => changes++);
  return { agentTab, tabs, control, views, changes: () => changes };
}

it('fences nothing while the agent has no tab yet, though a connected desktop sits in agent mode', () => {
  const { agentTab } = setup();
  assert.deepEqual([agentTab.humanMay(1), agentTab.humanMay(2), agentTab.humanMay(null)], [true, true, true]);
});

it("takes the shown tab on the agent's first command, then fences only that tab", () => {
  const { agentTab, views, changes } = setup();
  assert.equal(agentTab.drive(), views[1]);
  assert.deepEqual([agentTab.humanMay(1), agentTab.humanMay(2), agentTab.humanMay(null)], [false, true, true]);
  assert.equal(changes(), 1, 'the windows hear the claim');
});

it('keeps driving its own tab while the person shows another', () => {
  const { agentTab, tabs, views } = setup();
  agentTab.drive();
  tabs.activeTabId = 2;
  assert.equal(agentTab.view(), views[1]);
  assert.equal(agentTab.following(), false);
  assert.equal(agentTab.tabId(), 1);
});

it("answers no page once the agent's tab is gone, never the person's", () => {
  const { agentTab, tabs } = setup();
  agentTab.claim(7);
  tabs.activeTabId = 2;
  assert.equal(agentTab.view(), null);
});

it('local automation and another operator still take the whole browser', () => {
  for (const state of [
    { ...AGENT, localClients: 1 },
    { mode: 'human', mine: false, interactive: false, connected: true, localClients: 0 },
  ]) {
    const { agentTab } = setup(state);
    agentTab.drive();
    assert.deepEqual([agentTab.humanMay(1), agentTab.humanMay(2)], [false, false]);
  }
});

it('taking control ends the claim, and a run that ends lets the tab go', () => {
  const { agentTab, control } = setup();
  agentTab.drive();
  agentTab.release();
  assert.equal(agentTab.id, null);
  agentTab.drive();
  control.state = { ...AGENT, mode: 'human', mine: true, interactive: true };
  agentTab.sync(control.snapshot() as any);
  assert.equal(agentTab.id, null);
  assert.equal(agentTab.humanMay(1), true);
});

it("refuses the person on the agent's tab with the switch-to-Human-Mode message", () => {
  const { agentTab } = setup();
  agentTab.drive();
  assert.throws(() => agentTab.requireFor(1), /Switch to Human Mode/);
  assert.doesNotThrow(() => agentTab.requireFor(2));
});

it('gives each agent its own tab: a second one beside the first gets a fresh background tab, the person keeps the rest', () => {
  const { agentTab, tabs } = setup();
  inLane('agent-1', () => agentTab.drive());
  tabs.activeTabId = 2; // the person turns to their own tab
  inLane('agent-2', () => agentTab.drive());
  assert.deepEqual(tabs.created, ['oya:home'], 'opened in the background, never on the person');
  assert.deepEqual([inLane('agent-1', () => agentTab.id), inLane('agent-2', () => agentTab.id)], [1, 3]);
  assert.deepEqual([agentTab.humanMay(1), agentTab.humanMay(2), agentTab.humanMay(3)], [false, true, false]);
  assert.equal(
    inLane('agent-2', () => agentTab.othersDrive(1)),
    true,
  );
  assert.equal(
    inLane('agent-1', () => agentTab.othersDrive(1)),
    false,
  );
});

it("ends one agent's run without touching another's tab", () => {
  const { agentTab, tabs } = setup();
  inLane('agent-1', () => agentTab.drive());
  tabs.activeTabId = 2;
  inLane('agent-2', () => agentTab.drive());
  inLane('agent-1', () => agentTab.release());
  assert.deepEqual(agentTab.claimed(), [3]);
  assert.equal(agentTab.humanMay(1), true);
});

it("gives a quiet agent's tab back to the person: an API or MCP agent never says it finished", () => {
  mock.timers.enable({ apis: ['setTimeout'] });
  try {
    const { agentTab } = setup();
    inLane('mcp-agent', () => agentTab.drive());
    mock.timers.tick(LANE_IDLE_MS - 1);
    inLane('mcp-agent', () => agentTab.drive()); // still working
    mock.timers.tick(LANE_IDLE_MS - 1);
    assert.deepEqual(agentTab.claimed(), [1]);
    mock.timers.tick(1);
    assert.deepEqual(agentTab.claimed(), []);
    assert.equal(agentTab.humanMay(1), true);
  } finally {
    mock.timers.reset();
  }
});

it("a link the agent's page opens in a new tab stays the agent's, and the old tab is the person's again", () => {
  const { agentTab } = setup();
  agentTab.drive();
  agentTab.handOff(1, 2);
  assert.deepEqual([agentTab.humanMay(1), agentTab.humanMay(2)], [true, false]);
  agentTab.handOff(1, 3);
  assert.deepEqual(agentTab.claimed(), [2], 'a tab no agent drives hands nothing on');
});

it('an agent may work in its first tab and the tabs it opened, never the person’s other tabs', () => {
  const { agentTab } = setup();
  inLane('chat', () => {
    agentTab.claim(1); // the tab the chat was sent from
    const opened = agentTab.opened(agentTab['deps'].tabs.createTab('https://jobs.test/', false));
    assert.equal(agentTab.mayWorkIn(1), true, 'its first tab');
    assert.equal(agentTab.mayWorkIn(opened), true, 'a tab it opened');
    assert.equal(agentTab.mayWorkIn(2), false, 'the person’s other tab');
  });
});

it('another agent’s tabs are not this agent’s to work in', () => {
  const { agentTab } = setup();
  const theirs = inLane('a', () => agentTab.opened(agentTab['deps'].tabs.createTab('https://a.test/', false)));
  inLane('b', () => assert.equal(agentTab.mayWorkIn(theirs), false));
});

it('a finished run gives its first tab back, so the next run starts afresh', () => {
  const { agentTab } = setup();
  inLane('chat', () => {
    agentTab.claim(1);
    agentTab.release();
    assert.equal(agentTab.mayWorkIn(1), false);
  });
});

it('after a lane is released, the next lane takes the shown tab, not a new background one', () => {
  const { agentTab, tabs } = setup();
  inLane('sign-in', () => agentTab.drive());
  inLane('sign-in', () => agentTab.release());
  inLane('request', () => agentTab.drive());
  assert.deepEqual(agentTab.claimed(), [1]);
  assert.deepEqual(tabs.created, []);
});

it("a released lane's lapse is cleared, so finished lanes leave no timers behind", () => {
  mock.timers.enable({ apis: ['setTimeout'] });
  try {
    const { agentTab } = setup();
    inLane('sign-in', () => agentTab.drive());
    inLane('sign-in', () => agentTab.release());
    // A leak shows nowhere else: one timer and one entry left behind per finished lane.
    assert.equal((agentTab as any).lapses.size, 0);
  } finally {
    mock.timers.reset();
  }
});

it('a finished run closes the tabs it opened and brings a watching person back to the tab it started in', () => {
  const { agentTab, tabs } = setup();
  agentTab.drive();
  const opened = agentTab.opened(tabs.createTab('https://a.test', true));
  tabs.activeTabId = opened;
  agentTab.release();
  assert.deepEqual([tabs.closed, tabs.activeTabId], [[opened], 1]);
});

it('a finished run leaves the person where they are when they were not watching it', () => {
  const { agentTab, tabs } = setup();
  agentTab.drive();
  agentTab.opened(tabs.createTab('https://a.test', false));
  tabs.activeTabId = 2;
  agentTab.release();
  assert.equal(tabs.activeTabId, 2);
  assert.equal(tabs.closed.length, 1);
});

it("a finished run closes only its own tabs, never another agent's or the person's", () => {
  const { agentTab, tabs } = setup();
  inLane('a', () => agentTab.drive());
  const theirs = inLane('b', () => agentTab.opened(tabs.createTab('https://b.test', false)));
  inLane('a', () => agentTab.release());
  assert.equal(tabs.closed.includes(theirs), false);
  assert.equal(tabs.closed.includes(2), false);
});

it("an SDK browser's lane opens a tab of its own and leaves the person's shown tab alone", () => {
  const { agentTab, tabs } = setup();
  inLane('sdk-script', () => agentTab.drive());
  assert.equal(tabs.created.length, 1);
  assert.equal(tabs.created[0].endsWith(' shown'), true);
  assert.deepEqual(agentTab.claimed(), [3]);
  assert.equal(agentTab.humanMay(1), true);
});

it('an SDK lane beside another agent opens its tab in the background', () => {
  const { agentTab, tabs } = setup();
  inLane('mcp-agent', () => agentTab.drive());
  inLane('sdk-script', () => agentTab.drive());
  assert.equal(tabs.created.length, 1);
  assert.equal(tabs.created[0].endsWith(' shown'), false);
});

it('every call of one SDK lane stays in its one tab', () => {
  const { agentTab, tabs } = setup();
  inLane('sdk-script', () => agentTab.drive());
  inLane('sdk-script', () => agentTab.drive());
  inLane('sdk-script', () => agentTab.drive());
  assert.equal(tabs.created.length, 1);
  assert.deepEqual(agentTab.claimed(), [3]);
});

it('an ended SDK lane closes the tab it worked in', () => {
  const { agentTab, tabs } = setup();
  inLane('sdk-script', () => agentTab.drive());
  inLane('sdk-script', () => agentTab.release());
  assert.deepEqual(tabs.closed, [3]);
  assert.deepEqual(agentTab.claimed(), []);
});

it('an SDK lane that went quiet takes its own tab back, not a second one', () => {
  mock.timers.enable({ apis: ['setTimeout'] });
  try {
    const { agentTab, tabs } = setup();
    inLane('sdk-script', () => agentTab.drive());
    mock.timers.tick(LANE_IDLE_MS);
    assert.deepEqual(agentTab.claimed(), []);
    inLane('sdk-script', () => agentTab.drive());
    assert.equal(tabs.created.length, 1);
    assert.deepEqual(agentTab.claimed(), [3]);
  } finally {
    mock.timers.reset();
  }
});
