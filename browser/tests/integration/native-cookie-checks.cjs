/** Native cookie state is private, authoritative, HttpOnly-aware and independent from page-visible storage. */
const assert = require('node:assert/strict');
/** Exercise cookie management over the authenticated external boundary, never page script cookie emulation. */
module.exports = async function cookieChecks(a, b, context, second, session, session2, url, control) {
  const params = { browserContextId: context };
  const read = () => a.call('Storage.getCookies', params);
  const evaluate = (expression, extra = {}) =>
    a.call('Runtime.evaluate', { expression, returnByValue: true, ...extra }, session);
  await assert.rejects(a.call('Storage.getCookies'), /browserContextId/);
  await assert.rejects(b.call('Storage.getCookies', params), /foreign/);
  await assert.rejects(a.call('Storage.clearCookies', params, session), /browser endpoint/);
  await a.call('Storage.setCookies', {
    ...params,
    cookies: [
      { url, name: 'agent_auth', value: 'synthetic', httpOnly: true, sameSite: 'Lax', expires: -1 },
      { url, name: 'scoped', value: 'path-only', path: '/narrow' },
      { url, name: 'persistent', value: 'yes', expires: Math.floor(Date.now() / 1000) + 3600 },
    ],
  });
  const cookies = (await read()).cookies;
  const auth = cookies.find((c) => c.name === 'agent_auth');
  assert.equal(auth.httpOnly, true);
  assert.equal(auth.session, true);
  assert.equal(auth.expires, -1);
  assert.equal(auth.sameSite, 'Lax');
  assert.equal(auth.priority, 'Medium');
  assert.equal(auth.sourceScheme, 'NonSecure');
  assert.equal(auth.sourcePort, Number(new URL(url).port));
  assert.equal(cookies.find((c) => c.name === 'persistent').session, false);
  assert.deepEqual((await a.call('Storage.getCookies', { browserContextId: second })).cookies, []);
  assert.equal((await evaluate('document.cookie')).result.value.includes('agent_auth'), false);
  assert.equal((await evaluate('document.cookie')).result.value.includes('scoped'), false);
  const echo = (await evaluate('fetch("/cookie-echo").then(r=>r.text())', { awaitPromise: true })).result.value;
  assert.ok(echo.includes('agent_auth=synthetic'));
  assert.ok(!echo.includes('scoped='));
  await evaluate('fetch("/cookie-server").then(r=>r.text())', { awaitPromise: true });
  assert.equal((await read()).cookies.find((c) => c.name === 'server_priority').priority, 'High');
  await assert.rejects(
    a.call('Storage.setCookies', {
      ...params,
      cookies: [
        { url, name: 'must_not_write', value: 'x' },
        { url, name: 'invalid', value: 'x', partitionKey: {} },
      ],
    }),
    /Unsupported/,
  );
  assert.ok(!(await read()).cookies.some((c) => c.name === 'must_not_write'));
  await a.call('Oya.deleteCookie', { ...params, url, name: 'agent_auth' });
  assert.ok(!(await read()).cookies.some((c) => c.name === 'agent_auth'));
  assert.ok((await read()).cookies.some((c) => c.name === 'persistent'));
  control.localHeld = true;
  try {
    await assert.rejects(read(), /authorized/);
  } finally {
    control.localHeld = false;
  }
  await evaluate('localStorage.setItem("cookie_cleanup_oracle","kept")');
  await a.call('Storage.setCookies', {
    browserContextId: second,
    cookies: [{ url, name: 'other_context', value: 'kept' }],
  });
  await a.call('Storage.clearCookies', params);
  assert.deepEqual((await read()).cookies, []);
  assert.equal((await evaluate('localStorage.getItem("cookie_cleanup_oracle")')).result.value, 'kept');
  assert.ok(
    (await a.call('Storage.getCookies', { browserContextId: second })).cookies.some((c) => c.name === 'other_context'),
  );
  await a.call('Storage.clearCookies', { browserContextId: second });
  console.log(
    'PASS: native cookie writes/reads, real provenance and High priority, HttpOnly delivery, path scope, private-context isolation, targeted deletion, cookie-only clear and human-control refusal',
  );
};
