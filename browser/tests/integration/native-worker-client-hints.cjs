/** Preserve the engine's worker hint-denial policy while verifying real worker traffic and native identity. */
const assert = require('node:assert/strict');
const enabled = process.env.OYA_CHECK_WORKER_HINTS === '1';
/** Worker code captures its original request and native navigator before handling any page messages. */
function serve(req, res, path) {
  if (!path.startsWith('/worker/') || !path.endsWith('.js')) return false;
  res.setHeader('Content-Type', 'application/javascript');
  res.setHeader('Service-Worker-Allowed', '/worker/');
  const cross = `http://localhost:${req.socket.localPort}/echo`;
  const first = `const first={script:${JSON.stringify(req.headers)},ua:navigator.userAgent,low:navigator.userAgentData.toJSON()};`;
  const read = `async function read(){const same=await fetch('/echo').then(r=>r.json());const cross=await fetch(${JSON.stringify(cross)}).then(r=>r.json());return {...first,same,cross}};`;
  const handlers = {
    '/worker/dedicated.js': 'read().then(value=>postMessage(value));',
    '/worker/shared.js': 'self.onconnect=e=>read().then(value=>{e.ports[0].postMessage(value);e.ports[0].close()});',
    '/worker/service.js': `self.oninstall=()=>self.skipWaiting();self.onactivate=e=>e.waitUntil(clients.claim());self.onmessage=e=>e.waitUntil(read().then(value=>e.ports[0].postMessage(value)));self.onfetch=e=>{if(new URL(e.request.url).pathname.endsWith('/snapshot'))e.respondWith(read().then(value=>Response.json(value)))};`,
  };
  if (!Object.hasOwn(handlers, path)) {
    res.writeHead(404);
    res.end();
    return true;
  }
  res.end(first + read + handlers[path]);
  return true;
}
/** All worker types share native values but no hint headers may bypass the engine's all-blocking worker policy. */
function check(actual, jar, identity, label) {
  assert.equal(actual.ua, jar.getUserAgent(), label + ' navigator');
  assert.deepEqual(
    actual.low,
    { brands: identity.brands, mobile: identity.mobile, platform: identity.platform },
    label + ' metadata',
  );
  for (const surface of ['script', 'same', 'cross']) {
    assert.equal(actual[surface]['user-agent'], jar.getUserAgent(), label + ' ' + surface);
    assert.deepEqual(
      Object.keys(actual[surface]).filter((key) => key.startsWith('sec-ch-ua')),
      [],
      label + ' ' + surface + ' hints',
    );
  }
}
/** Exercise classic and module workers without suppressing errors or waiting for an unrelated registration. */
async function capture(window, phase, module) {
  const type = module ? 'module' : 'classic';
  const suffix = `?phase=${phase}-${type}`;
  const options = JSON.stringify({ type });
  const read = (source) => window.webContents.executeJavaScript(source);
  const dedicated = await read(
    `new Promise((resolve,reject)=>{const w=new Worker('/worker/dedicated.js${suffix}',${options});w.onmessage=e=>{w.terminate();resolve(e.data)};w.onerror=e=>{w.terminate();reject(Error(e.message))}})`,
  );
  const shared = await read(
    `new Promise((resolve,reject)=>{const w=new SharedWorker('/worker/shared.js${suffix}',${options});w.port.onmessage=e=>{w.port.close();resolve(e.data)};w.onerror=e=>reject(Error(e.message))})`,
  );
  const scope = `/worker/${phase}-${type}/`;
  const service = await read(
    `(async()=>{const r=await navigator.serviceWorker.register('/worker/service.js${suffix}',{type:${JSON.stringify(type)},scope:${JSON.stringify(scope)}});const w=r.installing||r.waiting||r.active;await new Promise((resolve,reject)=>{const ready=()=>{if(w.state==='activated')resolve();else if(w.state==='redundant')reject(Error('Service worker became redundant'))};w.addEventListener('statechange',ready);ready()});return new Promise(resolve=>{const c=new MessageChannel();c.port1.onmessage=e=>{c.port1.close();resolve(e.data)};r.active.postMessage('read',[c.port2])})})()`,
  );
  return { values: { dedicated, shared, service }, scope };
}
/** Parent opt-in and denial must not widen worker permissions or leak another partition's values. */
async function verify({ jarFor, windowFor, visit, url, metadata }) {
  if (!enabled) return;
  const identity = { ...metadata, platform: 'WorkerFixtureOS', architecture: 'x86' };
  const jar = jarFor('worker-hints-one');
  const sibling = jarFor('worker-hints-two', identity);
  const window = windowFor(jar);
  const other = windowFor(sibling);
  await visit(other, url + '/opt');
  for (const phase of ['first', 'opt', 'deny']) {
    await visit(window, url + '/' + phase);
    for (const module of [false, true]) {
      const result = await capture(window, phase, module);
      for (const [kind, actual] of Object.entries(result.values))
        check(actual, jar, metadata, `${phase} ${module} ${kind}`);
      const foreign = await capture(other, phase, module);
      for (const [kind, actual] of Object.entries(foreign.values))
        check(actual, sibling, identity, `sibling ${phase} ${module} ${kind}`);
    }
  }
  window.destroy();
  other.destroy();
  for (const [owner, ownIdentity] of [
    [jar, metadata],
    [sibling, identity],
  ]) {
    await owner.serviceWorkers._stopAllWorkers();
    for (const type of ['classic', 'module']) {
      const scope = url + `/worker/deny-${type}/`;
      await owner.serviceWorkers.startWorkerForScope(scope);
      const resumed = windowFor(owner);
      await visit(resumed, scope);
      const actual = await resumed.webContents.executeJavaScript(
        `fetch(${JSON.stringify(scope + 'snapshot')}).then(r=>r.json())`,
      );
      check(actual, owner, ownIdentity, 'cold service ' + type);
      resumed.destroy();
    }
    await owner.clearStorageData();
    const fresh = windowFor(owner);
    await visit(fresh, url + '/after-worker-clear');
    const result = await capture(fresh, 'after-clear', false);
    for (const [kind, actual] of Object.entries(result.values))
      check(actual, owner, ownIdentity, 'after clear ' + kind);
    fresh.destroy();
  }
  console.log(
    'PASS native worker traffic: classic/module dedicated/shared/service workers, actual script and same/cross-origin fetch headers, parent opt-in/denial, distinct sessions, cold service restart and storage clearing; worker UA hints remain denied',
  );
}
module.exports = { serve, verify };
