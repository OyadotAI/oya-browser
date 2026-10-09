/** Selective native interception and atomic filter changes are verified against real server request counts. */
const assert = require('node:assert/strict');
/** Await native request-paused delivery rather than scanning page text. */
async function until(read) {
  for (let i = 0; i < 500; i++) {
    const result = read();
    if (result) return result;
    await new Promise((r) => setTimeout(r, 10));
  }
  throw Error('Native selective pause did not arrive');
}
/** Filters never proxy requests, replace response bytes or release already-held work during updates. */
module.exports = async function patterns(a, session, hits) {
  const evaluate = (expression, extra = {}) =>
    a.call('Runtime.evaluate', { expression, returnByValue: true, ...extra }, session);
  const paused = (path) =>
    until(
      () => a.events.find((e) => e.method === 'Fetch.requestPaused' && e.params.request.url.endsWith(path))?.params,
    );
  await a.call('Fetch.enable', { patterns: [{ urlPattern: '*/filter/hold?', resourceType: 'XHR' }] }, session);
  assert.equal(
    (await evaluate('fetch("/filter/free").then(r=>r.text())', { awaitPromise: true })).result.value,
    '/filter/free',
  );
  await evaluate('window.filterPending=fetch("/filter/hold1").then(r=>r.text());"started"');
  const held = await paused('/filter/hold1');
  assert.equal(hits['/filter/hold1'] || 0, 0);
  await a.call('Fetch.enable', { patterns: [] }, session);
  assert.equal(hits['/filter/hold1'] || 0, 0, 'updating filters must not release held callbacks');
  assert.equal(
    (await evaluate('fetch("/filter/hold2").then(r=>r.text())', { awaitPromise: true })).result.value,
    '/filter/hold2',
  );
  await a.call('Fetch.continueRequest', { requestId: held.requestId }, session);
  assert.equal((await evaluate('filterPending', { awaitPromise: true })).result.value, '/filter/hold1');
  await a.call('Fetch.enable', { patterns: [{ urlPattern: '*/filter/pixel', resourceType: 'Image' }] }, session);
  await assert.rejects(a.call('Fetch.enable', { patterns: [{ requestStage: 'Response' }] }, session), /Request stage/);
  await evaluate('fetch("/filter/pixel").then(r=>r.text())', { awaitPromise: true });
  assert.equal(hits['/filter/pixel'], 1, 'resource mismatch must pass normally');
  const image = evaluate(
    'new Promise((resolve,reject)=>{const image=new Image();image.onload=()=>resolve("loaded");image.onerror=()=>reject("image failed");image.src="/filter/pixel"})',
    { awaitPromise: true },
  );
  const pixel = await paused('/filter/pixel');
  assert.equal(pixel.resourceType, 'Image');
  assert.equal(hits['/filter/pixel'], 1, 'invalid update must leave the Image rule active');
  await a.call('Fetch.continueRequest', { requestId: pixel.requestId }, session);
  assert.equal((await image).result.value, 'loaded');
  assert.equal(hits['/filter/pixel'], 2);
  await a.call('Fetch.disable', {}, session);
  console.log(
    'PASS: native URL/resource filters, nonmatching request delivery, empty rules, atomic live replacement, preserved held callbacks and failed-update rollback',
  );
};
