/** Unit tests for scripts/indexnow.mjs: what it reads from a sitemap and what it sends. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { KEY, runIndexNow, sitemapUrls, submission } from './indexnow.mjs';
import { INDEXNOW_REQUEST_TIMEOUT_MS } from './constants.mjs';

/** A representative sitemap contains more than one URL so payload counts are meaningful. */
const SITE = 'https://oyabrowser.com';
const XML = `<urlset><url><loc>${SITE}/</loc></url><url><loc>${SITE}/pricing</loc></url></urlset>`;

/** Records each request while returning isolated responses without accessing the network. */
function fakeNetwork(...responses) {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, ...init });
    const response = responses.shift();
    if (response instanceof Error) throw response;
    return typeof response === 'function' ? response(init) : response;
  };
  return { calls, fetchImpl };
}

/** Simulates native fetch or response consumption rejecting when its request is aborted. */
function untilAborted(signal) {
  return new Promise((_, reject) => signal.addEventListener('abort', () => reject(signal.reason), { once: true }));
}

/** Runs the real CLI with a preload seam so stdout, stderr and exit status stay hermetic. */
function cli(preload, ...args) {
  return spawnSync(
    process.execPath,
    [
      '--import',
      `data:text/javascript,${encodeURIComponent(preload)}`,
      new URL('./indexnow.mjs', import.meta.url).pathname,
      ...args,
    ],
    {
      encoding: 'utf8',
      env: { ...process.env, SITE_URL: SITE },
    },
  );
}

test('reads every <loc> from a sitemap, in order', () => {
  const xml = '<urlset><url><loc>https://a.com/</loc></url><url><loc>\n https://a.com/ai-browser </loc></url></urlset>';
  assert.deepEqual(sitemapUrls(xml), ['https://a.com/', 'https://a.com/ai-browser']);
});

test('sends the host, the key and where the key file lives', () => {
  assert.deepEqual(submission('https://oyabrowser.com', ['https://oyabrowser.com/']), {
    host: 'oyabrowser.com',
    key: KEY,
    keyLocation: `https://oyabrowser.com/${KEY}.txt`,
    urlList: ['https://oyabrowser.com/'],
  });
});

test('the site serves a key file holding exactly the key', () => {
  assert.equal(readFileSync(new URL(`../ui/public/${KEY}.txt`, import.meta.url), 'utf8'), KEY);
});

test('submits every live sitemap URL once and returns the accepted response status', async () => {
  const { calls, fetchImpl } = fakeNetwork(new Response(XML), new Response(null, { status: 202 }));
  const result = await runIndexNow({ site: SITE, fetchImpl });
  assert.deepEqual(result, { payload: submission(SITE, [`${SITE}/`, `${SITE}/pricing`]), status: 202 });
  assert.deepEqual(
    calls.map(({ url, method }) => [url, method]),
    [
      [`${SITE}/sitemap.xml`, undefined],
      ['https://api.indexnow.org/indexnow', 'POST'],
    ],
  );
  assert.deepEqual(JSON.parse(calls[1].body), result.payload);
});

test('dry-run returns the existing payload without sending a notification', async () => {
  const { calls, fetchImpl } = fakeNetwork(new Response(XML));
  assert.deepEqual(await runIndexNow({ site: SITE, dryRun: true, fetchImpl }), {
    payload: submission(SITE, [`${SITE}/`, `${SITE}/pricing`]),
  });
  assert.equal(calls.length, 1);
});

test('a rejected sitemap fails without submitting or retaining its response body', async () => {
  const response = new Response('private upstream error', { status: 503 });
  const { calls, fetchImpl } = fakeNetwork(response);
  await assert.rejects(runIndexNow({ fetchImpl }), {
    message: 'IndexNow: sitemap fetch failed (HTTP 503); no URLs submitted.',
  });
  assert.equal(calls.length, 1);
  assert.equal(response.bodyUsed, true);
});

test('an empty sitemap fails before any notification is sent', async () => {
  const { calls, fetchImpl } = fakeNetwork(new Response('<urlset/>'));
  await assert.rejects(runIndexNow({ fetchImpl }), {
    message: 'IndexNow: sitemap contains no URLs; no submission sent.',
  });
  assert.equal(calls.length, 1);
});

test('a rejected notification reports its status and URL count without logging the body', async () => {
  const response = new Response('private upstream error', { status: 422 });
  const { fetchImpl } = fakeNetwork(new Response(XML), response);
  await assert.rejects(runIndexNow({ fetchImpl }), { message: 'IndexNow: submission failed (HTTP 422) for 2 URLs.' });
  assert.equal(response.bodyUsed, true);
});

for (const stage of ['sitemap fetch', 'submission']) {
  test(`a ${stage} disconnect reports the failed stage`, async () => {
    const responses = stage === 'submission' ? [new Response(XML)] : [];
    const { fetchImpl } = fakeNetwork(...responses, new Error('connection reset'));
    await assert.rejects(runIndexNow({ fetchImpl }), { message: `IndexNow: ${stage} failed: connection reset.` });
  });
}

for (const stage of ['sitemap fetch', 'submission']) {
  test(`the ${stage} deadline aborts waiting for response headers`, async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const fetchImpl = async (_, { signal }) => untilAborted(signal);
    const run = runIndexNow({
      fetchImpl:
        stage === 'submission'
          ? async (url, init) => (url.endsWith('/sitemap.xml') ? new Response(XML) : fetchImpl(url, init))
          : fetchImpl,
    });
    const rejected = assert.rejects(run, { message: `IndexNow: ${stage} timed out after 15s.` });
    if (stage === 'submission') await new Promise(setImmediate);
    t.mock.timers.tick(INDEXNOW_REQUEST_TIMEOUT_MS);
    await rejected;
  });
}

test('the sitemap deadline includes consuming the response body', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const fetchImpl = async (_, { signal }) => ({ ok: true, text: () => untilAborted(signal) });
  const rejected = assert.rejects(runIndexNow({ fetchImpl }), {
    message: 'IndexNow: sitemap fetch timed out after 15s.',
  });
  await new Promise(setImmediate);
  t.mock.timers.tick(INDEXNOW_REQUEST_TIMEOUT_MS);
  await rejected;
});

test('the submission deadline includes releasing its unused response body', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const response = ({ signal }) => ({ ok: true, status: 200, body: { cancel: () => untilAborted(signal) } });
  const { fetchImpl } = fakeNetwork(new Response(XML), response);
  const rejected = assert.rejects(runIndexNow({ fetchImpl }), { message: 'IndexNow: submission timed out after 15s.' });
  await new Promise(setImmediate);
  t.mock.timers.tick(INDEXNOW_REQUEST_TIMEOUT_MS);
  await rejected;
});

for (const fails of [false, true]) {
  test(`request timers are cleared after ${fails ? 'failure' : 'success'}`, async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const { calls, fetchImpl } = fakeNetwork(new Response(XML), fails ? new Error('connection reset') : new Response());
    const run = runIndexNow({ fetchImpl });
    if (fails) await assert.rejects(run);
    else await run;
    t.mock.timers.tick(INDEXNOW_REQUEST_TIMEOUT_MS);
    assert.ok(calls.every(({ signal }) => !signal.aborted));
  });
}

test('the CLI reports notification acceptance with count and HTTP status', () => {
  const result = cli(
    `globalThis.fetch = async url => new Response(url.endsWith('/sitemap.xml') ? ${JSON.stringify(XML)} : null);`,
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, 'IndexNow: 200 for 2 URLs (notification received)\n');
  assert.equal(result.stderr, '');
});

test('the CLI preserves dry-run JSON stdout and sends no POST', () => {
  const result = cli(
    `globalThis.fetch = async (_, init) => { if (init.method) throw Error('unexpected POST'); return new Response(${JSON.stringify(XML)}); };`,
    '--dry-run',
  );
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), submission(SITE, [`${SITE}/`, `${SITE}/pricing`]));
  assert.equal(result.stderr, '');
});

test('the CLI returns a nonzero exit status and an actionable error on failure', () => {
  const result = cli('globalThis.fetch = async () => new Response(null, { status: 503 });');
  assert.equal(result.status, 1);
  assert.equal(result.stdout, '');
  assert.equal(result.stderr, 'IndexNow: sitemap fetch failed (HTTP 503); no URLs submitted.\n');
});
