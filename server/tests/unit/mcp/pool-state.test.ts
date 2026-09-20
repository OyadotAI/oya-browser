/**
 * Unit tests for which browser a pool agent drives: pinned between page
 * contexts, advanced on navigation, sticky once started, and cleared when a
 * browser stops or disconnects.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  browserTag,
  destroyMcpServer,
  hold,
  letGo,
  pickFor,
  poolPinned,
  poolSticky,
} from '../../../src/mcp/pool-state.ts';
import { connectBrowser, disconnectBrowser } from '../support/fakes.ts';

const KEY = 'k-pin';

describe('pool state', () => {
  afterEach(() => {
    ['m-1', 'm-2'].forEach(disconnectBrowser);
    poolPinned.clear();
    poolSticky.clear();
  });

  it('has no browser to pick for an empty pool', () => {
    assert.equal(pickFor(KEY)(false), null);
  });

  it('stays on the pinned browser for commands in the same page context', () => {
    connectBrowser('m-1', KEY);
    connectBrowser('m-2', KEY);
    const pick = pickFor(KEY);
    const first = pick(false);
    assert.equal(pick(false), first);
    assert.equal(pick(false), first);
  });

  it('moves on to the next browser for a new page context', () => {
    connectBrowser('m-1', KEY);
    connectBrowser('m-2', KEY);
    const pick = pickFor(KEY);
    const first = pick(false);
    assert.notEqual(pick(true), first);
  });

  it('keeps a held browser even for a new page context', () => {
    connectBrowser('m-1', KEY);
    connectBrowser('m-2', KEY);
    hold(KEY, 'm-2');
    const pick = pickFor(KEY);
    assert.equal(pick(true), 'm-2');
    assert.equal(pick(true), 'm-2');
  });

  it('re-picks when the pinned browser has gone', () => {
    connectBrowser('m-1', KEY);
    poolPinned.set(KEY, 'gone');
    assert.equal(pickFor(KEY)(false), 'm-1');
  });

  it('lets go only of the browser named', () => {
    hold(KEY, 'm-1');
    letGo(KEY, 'm-2');
    assert.equal(poolPinned.get(KEY), 'm-1');
    letGo(KEY, 'm-1');
    assert.equal(poolPinned.has(KEY), false);
    assert.equal(poolSticky.has(KEY), false);
  });

  it('clears a disconnected browser from every key’s pins', () => {
    hold(KEY, 'm-1');
    poolPinned.set('other', 'm-1');
    destroyMcpServer('m-1');
    assert.equal(poolPinned.size, 0);
    assert.equal(poolSticky.size, 0);
  });

  it('tags output with the browser’s name, or just its id once gone', () => {
    connectBrowser('m-1', KEY);
    assert.equal(browserTag('m-1'), '[Test m-1]');
    assert.equal(browserTag('gone'), '[gone]');
  });
});
