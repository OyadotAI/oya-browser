/** External clients preserve origin isolation and never invent human activation to evaluate scripts. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fakeDriver } from '../../support/cdp.ts';

test('isolated evaluation requests neither universal origins nor synthetic user gestures', async () => {
  const { driver, conn } = fakeDriver();
  await driver.evaluate('42');
  assert.equal(Object.hasOwn(conn.sent('Page.createIsolatedWorld')[0].params, 'grantUniveralAccess'), false);
  for (const call of conn.sent('Runtime.evaluate')) assert.equal(Object.hasOwn(call.params, 'userGesture'), false);
});

test('main evaluation omits an absent context rather than sending unsupported metadata', async () => {
  const { driver, conn } = fakeDriver();
  await driver.evaluateMain('42');
  assert.equal(Object.hasOwn(conn.sent('Runtime.evaluate')[0].params, 'contextId'), false);
  assert.equal(Object.hasOwn(conn.sent('Runtime.evaluate')[0].params, 'userGesture'), false);
});

test('failed analyzer installation does not publish a ready world', async () => {
  const { driver, conn } = fakeDriver();
  conn.replies['Runtime.evaluate'] = { exceptionDetails: { exception: { description: 'installation failed' } } };
  await assert.rejects(driver.ensureWorld(), /installation failed/);
  assert.equal(driver.worldContext, null);
  conn.replies['Runtime.evaluate'] = { result: {} };
  await driver.ensureWorld();
  assert.equal(conn.sent('Page.createIsolatedWorld').length, 2);
});

test('a failed forced rebuild revokes the previously cached world', async () => {
  const { driver, conn } = fakeDriver();
  await driver.ensureWorld();
  conn.replies['Page.createIsolatedWorld'] = Error('Document replaced');
  await assert.rejects(driver.ensureWorld({ force: true }), /Document replaced/);
  assert.equal(driver.worldContext, null);
});
