/** Real authenticated external transport terminates in Oya native operations, with engine debugger access forbidden. */
import { createServer } from 'node:http';
import { once } from 'node:events';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CDPConnection } from '../../src/drivers/cdp/connection.ts';
import { CDPDriver } from '../../src/drivers/cdp/driver.ts';
import { openNativeFixture } from '../support/native-browser.mjs';

test('external connection authenticates, attaches and evaluates through the native front door', async () => {
  const site = createServer((_req, res) =>
    res.end(`<!doctype html><title>Native connection</title>
    <form onsubmit="event.preventDefault();window.submissions=(window.submissions||0)+1">
    <input id="search" aria-label="Search"><button>Search</button></form>
    <script>window.keyEvents=[];document.addEventListener('keydown',e=>keyEvents.push({key:e.key,trusted:e.isTrusted}))</script>`),
  );
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
    const navigation = await conn.send(
      'Page.navigate',
      { url: `http://127.0.0.1:${site.address().port}/?native-navigation=1` },
      sessionId,
    );
    assert.ok(navigation.frameId);
    const location = await conn.send('Runtime.evaluate', { expression: 'location.search' }, sessionId);
    assert.equal(location.result.value, '?native-navigation=1');
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
    const driver = new CDPDriver();
    Object.assign(driver, { conn, sessionId, targetId: targetInfos[0].targetId, tagAttr: 'data-native-test' });
    assert.equal(await driver.evaluate('6 * 7'), 42);
    assert.equal(await driver.evaluateMain('typeof analyzePage'), 'undefined');
    assert.equal((await driver.dispatch('type', { selector: '#search', text: 'Jordans 日本' })).ok, true);
    assert.equal(await driver.evaluateMain('document.querySelector("#search").value'), 'Jordans 日本');
    assert.equal((await driver.dispatch('press-key', { key: 'Enter' })).ok, true);
    assert.equal(await driver.evaluateMain('window.submissions'), 1);
    const keys = await driver.evaluateMain('keyEvents');
    assert.deepEqual(keys, [{ key: 'Enter', trusted: true }]);
    await driver.evaluateMain(
      'window.moves=[];document.addEventListener("mousemove",e=>moves.push({buttons:e.buttons,trusted:e.isTrusted}))',
    );
    await driver.dispatch('drag', { from_x: 20, from_y: 100, to_x: 200, to_y: 120 });
    const moves = await driver.evaluateMain('moves');
    assert.ok(moves.some((move) => move.buttons === 1 && move.trusted));
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
