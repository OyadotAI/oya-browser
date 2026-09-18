/**
 * Unit tests for dialling a browser's CDP socket: opened() settles on open or
 * error, and dialWithTimeout gives up on a browser that never answers.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import type { AddressInfo } from 'node:net';
import { dial, opened, dialWithTimeout } from '../../../../src/modules/gateway/upstream.ts';
import { UPSTREAM_CONNECT_MS } from '../../../../src/modules/gateway/constants.ts';
import { fakeCdp } from '../../support/gateway.ts';

describe('upstream', () => {
  afterEach(() => mock.timers.reset());

  it('opens a socket to a listening browser', async () => {
    const browser = await fakeCdp();
    const ws = dial(browser.url);
    await opened(ws);
    assert.equal(ws.readyState, 1);
    ws.close();
    await browser.close();
  });

  it('rejects when nothing listens', async () => {
    const browser = await fakeCdp();
    await browser.close();
    await assert.rejects(opened(dial(browser.url)), /ECONNREFUSED/);
    await assert.rejects(dialWithTimeout(browser.url), /ECONNREFUSED/);
  });

  it('resolves with the open socket when dialled with a timeout', async () => {
    const browser = await fakeCdp();
    const ws = await dialWithTimeout(browser.url);
    assert.equal(ws.readyState, 1);
    ws.close();
    await browser.close();
  });

  it('gives up on a browser that accepts the connection but never answers', async () => {
    const sockets: any[] = [];
    const silent = createServer((s) => sockets.push(s));
    await new Promise<void>((resolve) => silent.listen(0, '127.0.0.1', resolve));
    mock.timers.enable({ apis: ['setTimeout'] });
    const dialling = dialWithTimeout(`ws://127.0.0.1:${(silent.address() as AddressInfo).port}/`);
    const settled = assert.rejects(dialling, /upstream connect timed out/);
    mock.timers.tick(UPSTREAM_CONNECT_MS);
    await settled;
    sockets.forEach((s) => s.destroy());
    await new Promise((resolve) => silent.close(resolve));
  });
});
