/** Real authenticated external transport terminates in Oya native operations, with engine debugger access forbidden. */
import { createServer } from 'node:http';
import { once } from 'node:events';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CDPConnection } from '../../src/drivers/cdp/connection.ts';
import { openNativeFixture } from '../support/native-browser.mjs';

test('external connection authenticates, attaches and evaluates through the native front door', async () => {
  const site = createServer((_req, res) => res.end('<!doctype html><title>Native connection</title>'));
  site.listen(0, '127.0.0.1');
  await once(site, 'listening');
  const browser = await openNativeFixture().catch((error) => {
    site.close();
    throw error;
  });
  const connections = [];
  try {
    await browser.prepareProfile({
      fingerprint: { id: 'native-connection' },
      origins: {},
      cookies: [],
      now: Date.now(),
    });
    await browser.send('navigate', { url: `http://127.0.0.1:${site.address().port}/` });
    const endpoint = await browser.frontDoor();
    for (const credentials of [{}, { bearerToken: 'incorrect' }]) {
      const denied = new CDPConnection(endpoint.url, credentials);
      connections.push(denied);
      await assert.rejects(denied.connect(), /401|403/);
    }
    const conn = new CDPConnection(endpoint.url, { bearerToken: endpoint.token });
    connections.push(conn);
    await conn.connect();
    const { targetInfos } = await conn.send('Target.getTargets');
    assert.equal(targetInfos.length, 1);
    const { sessionId } = await conn.send('Target.attachToTarget', {
      targetId: targetInfos[0].targetId,
      flatten: true,
    });
    const result = await conn.send(
      'Runtime.evaluate',
      { expression: '({answer:6*7,node:typeof process})', returnByValue: true },
      sessionId,
    );
    assert.deepEqual(result.result.value, { answer: 42, node: 'undefined' });
    const { frameTree } = await conn.send('Page.getFrameTree', {}, sessionId);
    const world = await conn.send(
      'Page.createIsolatedWorld',
      { frameId: frameTree.frame.id, worldName: 'server-agent' },
      sessionId,
    );
    await conn.send(
      'Runtime.evaluate',
      { contextId: world.executionContextId, expression: 'globalThis.privateAgentValue=42' },
      sessionId,
    );
    const isolated = await conn.send(
      'Runtime.evaluate',
      { contextId: world.executionContextId, expression: 'privateAgentValue' },
      sessionId,
    );
    assert.equal(isolated.result.value, 42);
    const main = await conn.send('Runtime.evaluate', { expression: 'typeof privateAgentValue' }, sessionId);
    assert.equal(main.result.value, 'undefined');
    await assert.rejects(conn.send('Page.createIsolatedWorld', { frameId: 'unsupported' }, sessionId), /foreign/i);
  } finally {
    for (const conn of connections) conn.close();
    await browser.close();
    await new Promise((resolve) => site.close(resolve));
  }
});

test('authenticated connections never follow a redirect to another endpoint', async () => {
  let redirected = false;
  const site = createServer((req, res) => {
    if (req.url === '/destination') redirected = true;
    res.writeHead(302, { Location: '/destination' }).end();
  });
  site.listen(0, '127.0.0.1');
  await once(site, 'listening');
  const conn = new CDPConnection(`ws://127.0.0.1:${site.address().port}/redirect`, {
    bearerToken: 'private-fixture-token',
  });
  try {
    await assert.rejects(conn.connect(), /302/);
    assert.equal(redirected, false);
  } finally {
    conn.close();
    await new Promise((resolve) => site.close(resolve));
  }
});
