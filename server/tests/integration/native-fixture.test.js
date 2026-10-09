/** The native fixture bridge must fail closed, return real page values, and stop cleanly on every exit. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { openNativeFixture } from '../support/native-browser.mjs';

test('a missing configured engine fails instead of skipping or choosing another browser', async () => {
  const previous = process.env.OYA_NATIVE_ENGINE;
  delete process.env.OYA_NATIVE_ENGINE;
  try {
    await assert.rejects(openNativeFixture(), /OYA_NATIVE_ENGINE/);
  } finally {
    if (previous !== undefined) process.env.OYA_NATIVE_ENGINE = previous;
  }
});

test('native fixture values, exceptions, unsupported commands and loopback-only navigation', async () => {
  const browser = await openNativeFixture();
  try {
    assert.deepEqual(await browser.evaluateMain('({answer:6*7,page:location.href})'), {
      answer: 42,
      page: 'about:blank',
    });
    await assert.rejects(browser.evaluateMain('throw Error("fixture exception")'), /JavaScript evaluation failed/);
    await assert.rejects(browser.send('unsupported'), /Unsupported native fixture action/);
    await assert.rejects(browser.send('navigate', { url: 'https://example.com/' }), /restricted to loopback/);
    assert.equal(await browser.evaluateMain('location.href'), 'about:blank');
    assert.equal(await browser.evaluateMain('typeof process'), 'undefined');
  } finally {
    await browser.close();
  }
  await assert.rejects(browser.evaluateMain('1'), /closed|exited|destroyed|write|ended/i);
  await browser.close();
});
