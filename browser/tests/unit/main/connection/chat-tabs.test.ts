/**
 * Unit tests for keeping a chat in its tab across a dropped socket: the tab each
 * chat's agent drove is given back once its claim is gone, the chat's own tab
 * when that one has closed, and never a tab an earlier run in the same tab drove.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { ChatTabs } from '../../../../src/main/connection/chat-tabs.ts';
import { currentLane } from '../../../../src/main/control/agent-tab.ts';

/** Agents' tabs by lane, the chats in flight and the open tabs, with every claim recorded. */
function fakes(open: number[]) {
  const claims = new Map<string, number>();
  const claimed: [string, number][] = [];
  const agentTab = {
    get id() {
      return claims.get(currentLane()) ?? null;
    },
    claim: (tab: number) => (claims.set(currentLane(), tab), claimed.push([currentLane(), tab])),
  };
  const chatRuns = new Map<number, AbortController>();
  const tabs = { find: (id: number) => (open.includes(id) ? { id } : undefined) };
  return { claims, claimed, chatRuns, chatTabs: new ChatTabs({ agentTab, chatRuns, tabs } as any) };
}

it('a chat whose claim a dropped socket ended gets back the tab its agent was driving', () => {
  const { claims, claimed, chatRuns, chatTabs } = fakes([3, 9]);
  chatRuns.set(3, new AbortController());
  claims.set('tab-3', 9);
  chatTabs.note();
  claims.clear();
  chatTabs.restore();
  assert.deepEqual(claimed, [
    ['tab-3', 3],
    ['tab-3', 9],
  ]);
});

it('goes back to the tab the chat was sent from when the tab it drove has closed', () => {
  const { claims, claimed, chatRuns, chatTabs } = fakes([3]);
  chatRuns.set(3, new AbortController());
  claims.set('tab-3', 9);
  chatTabs.note();
  claims.clear();
  chatTabs.restore();
  assert.deepEqual(claimed, [['tab-3', 3]]);
});

it('leaves an agent that has a tab where it is, whatever it drove before', () => {
  const { claims, claimed, chatRuns, chatTabs } = fakes([3, 9, 12]);
  chatRuns.set(3, new AbortController());
  claims.set('tab-3', 9);
  chatTabs.note();
  claims.set('tab-3', 12);
  chatTabs.restore();
  assert.deepEqual(claimed, []);
});

it('each chat gets its own tab back, in its own lane', () => {
  const { claims, claimed, chatRuns, chatTabs } = fakes([3, 8]);
  chatRuns.set(3, new AbortController());
  chatRuns.set(8, new AbortController());
  claims.set('tab-3', 3);
  claims.set('tab-8', 8);
  chatTabs.note();
  claims.clear();
  chatTabs.restore();
  assert.deepEqual(claimed, [
    ['tab-3', 3],
    ['tab-8', 8],
  ]);
});

it('the next run in a tab never gets the tab the run before it drove', () => {
  const { claims, claimed, chatRuns, chatTabs } = fakes([3, 9]);
  chatRuns.set(3, new AbortController());
  claims.set('tab-3', 9);
  chatTabs.note();
  claims.clear();
  chatRuns.set(3, new AbortController());
  chatTabs.restore();
  assert.deepEqual(claimed, [['tab-3', 3]]);
});
