/** Tests interoperability through the installed official MCP SDK and an explicitly isolated native window. */
import assert from 'node:assert/strict';
import console from 'node:console';
import process from 'node:process';
import { clearTimeout, setTimeout } from 'node:timers';
import { setTimeout as delay } from 'node:timers/promises';
import { URL } from 'node:url';

/** Resolve the existing dependency without installing clients or changing the repository lockfile. */
const sdk = new URL('../../../server/node_modules/@modelcontextprotocol/sdk/dist/esm/', import.meta.url);
const { Client } = await import(new URL('client/index.js', sdk));
const { StdioClientTransport } = await import(new URL('client/stdio.js', sdk));
/** Protocol waits and the overall owner deadline remain finite even after an assertion failure. */
const REQUEST_MS = 8000, OWNER_MS = 35000, POLL_MS = 25, WAIT_MS = 4000;
const TOOL_NAMES = ['runtime_capabilities', 'tab_list', 'control_status', 'page_inspect', 'page_summary',
  'page_find', 'page_diff', 'page_wait', 'page_act', 'page_layout',
  'page_navigate', 'page_type', 'page_click', 'page_set_checked', 'page_select', 'page_submit',
  'page_back', 'page_forward', 'page_reload', 'page_scroll', 'control_release'];
const client = new Client({ name: 'oya-native-engine-tests', version: '1' }, { capabilities: {} });
const transport = new StdioClientTransport({ command: process.argv[2],
  args: ['desktop', '--mcp', '--test-profile', '--verify-mcp-ui', '--url', 'demo'], stderr: 'pipe' });
let diagnostics = '', ownedChild, servicePids = [], tab, epoch;
const protocolErrors = [];
client.onerror = error => protocolErrors.push(error.message);
transport.stderr.on('data', bytes => { diagnostics = (diagnostics + bytes).slice(-8192); });

/** A watchdog targets only the process this test launched and never a user's existing preview. */
const watchdog = setTimeout(() => {
  ownedChild?.kill('SIGKILL');
  process.stderr.write('Official MCP SDK journey exceeded its owner deadline.\n');
  process.exit(1);
}, OWNER_MS);

/** All tools must provide the official result envelope and exact duplicate native JSON. */
async function result(name, args = {}) {
  const value = await client.callTool({ name, arguments: args }, undefined, { timeout: REQUEST_MS });
  assert.equal(value.content.length, 1);
  assert.equal(value.content[0].type, 'text');
  assert.deepEqual(JSON.parse(value.content[0].text), value.structuredContent);
  return value;
}

/** Success checks retain native results without adapter-generated success prose. */
async function call(name, args = {}) {
  const value = await result(name, args);
  assert.equal(value.isError, false, `native tool ${name} failed`);
  return value.structuredContent;
}

/** Failure assertions require the original native error classification. */
async function refused(name, args, code) {
  const value = await result(name, args);
  assert.equal(value.isError, true);
  assert.equal(value.structuredContent.error.code, code);
  return value.structuredContent.error;
}

/** Author IDs select synthetic controls only after inspection supplies their opaque native identities. */
function node(page, id) {
  const value = page.nodes.find(item => item.html_id === id);
  assert.ok(value, `synthetic control ${id} missing`);
  return value;
}

/** Every mutation copies the exact observed page revision and native toolbar grant. */
function target(page, id) {
  return { tab_id: tab, control_epoch: epoch, page_id: page.page_id,
    node_id: node(page, id).node_id, expected_revision: page.revision };
}

/** Poll only the explicit local toolbar harness transition, with a bounded test deadline. */
async function owner(expected) {
  const until = Date.now() + WAIT_MS;
  while (Date.now() < until) {
    const status = await call('control_status', { tab_id: tab });
    if (status.owner === expected) return status;
    await delay(POLL_MS);
  }
  assert.fail(`native toolbar did not reach ${expected}`);
}

/** Literal field edits refresh only observation between separate deliberate test actions. */
async function type(id, text) {
  const page = await call('page_inspect', { tab_id: tab });
  return call('page_type', { ...target(page, id), text });
}

/** The actual toolbar grants, takes over and grants again; MCP exposes no grant operation. */
async function toolbarJourney() {
  let page = await call('page_inspect', { tab_id: tab });
  await refused('page_type', { ...target(page, 'name'), text: 'denied' }, 'control_denied');
  epoch = (await owner('agent')).epoch;
  await type('name', 'MCP toolbar');
  page = await call('page_inspect', { tab_id: tab });
  const greeting = await call('page_click', target(page, 'greet'));
  assert.equal(node(greeting, 'result').text, 'Hello, MCP toolbar! From your own Rust V9.');
  await owner('human');
  await refused('page_click', target(greeting, 'greet'), 'control_denied');
  epoch = (await owner('agent')).epoch;
}

/** Checkbox, radio and select actions use discovered identities rather than author values. */
async function formChoices() {
  let page = await call('page_inspect', { tab_id: tab });
  await call('page_set_checked', { ...target(page, 'remember'), checked: false });
  page = await call('page_inspect', { tab_id: tab });
  await call('page_set_checked', { ...target(page, 'plus'), checked: true });
  page = await call('page_inspect', { tab_id: tab });
  const canada = page.nodes.find(item => item.tag === 'option' && item.text === 'Canada');
  assert.ok(canada);
  await call('page_select', { ...target(page, 'region'), option_id: canada.node_id });
  return call('page_inspect', { tab_id: tab });
}

/** Form validation, protected fields, stale revisions and synthetic cookie sessions traverse the same Runtime. */
async function formJourney(base) {
  await call('page_navigate', { tab_id: tab, control_epoch: epoch, url: new URL('forms', base).href });
  let page = await call('page_inspect', { tab_id: tab });
  await refused('page_submit', target(page, 'submit'), 'validation_error');
  for (const [id, text] of [['user', 'MCP Ada'], ['pass', 'synthetic-secret'], ['notes', 'First\nSecond']]) await type(id, text);
  page = await formChoices();
  assert.equal(node(page, 'remember').checked, false);
  assert.equal(node(page, 'plus').checked, true);
  assert.equal(node(page, 'standard').checked, false);
  assert.equal(Object.hasOwn(node(page, 'pass'), 'value'), false);
  await staleRevision(page);
  const account = await call('page_submit', target(page, 'submit'));
  assert.ok(account.url.endsWith('/account'));
  assert.ok(account.nodes.some(item => item.text === 'Your synthetic session is active.'));
  await released(account);
}

/** Refused stale calls cannot change the committed form or retarget a fresh page. */
async function staleRevision(page) {
  await refused('page_type', { ...target(page, 'user'), expected_revision: page.revision - 1, text: 'stale' }, 'revision_conflict');
  const after = await call('page_inspect', { tab_id: tab });
  assert.equal(after.revision, page.revision);
  assert.equal(node(after, 'user').value, 'MCP Ada');
  const layout = await call('page_layout', { tab_id: tab });
  assert.equal(layout.page_id, page.page_id);
  assert.equal(layout.unit, 'logical_points');
}

/** Yielding control advances authority and immediately refuses the previous grant. */
async function released(page) {
  const value = await call('control_release', { tab_id: tab, control_epoch: epoch });
  assert.equal(value.owner, 'human');
  await refused('page_reload', { tab_id: tab, control_epoch: epoch }, 'control_denied');
  assert.equal((await call('page_inspect', { tab_id: tab })).page_id, page.page_id);
}

/** Normal SDK shutdown must finish by EOF without the SDK's two-second signal fallback. */
async function closeCleanly() {
  const started = Date.now();
  const exit = new Promise(resolve => ownedChild.once('exit', (code, signal) => resolve({ code, signal })));
  await client.close();
  assert.deepEqual(await exit, { code: 0, signal: null });
  assert.ok(Date.now() - started < 2000, 'SDK needed its forced-termination fallback');
  for (const pid of servicePids) assert.throws(() => process.kill(pid, 0), { code: 'ESRCH' });
}

/** Real SDK initialization and discovery validate the fixed protocol and catalog before mutation. */
async function run() {
  await client.connect(transport, { timeout: REQUEST_MS });
  ownedChild = transport._process;
  assert.ok(ownedChild?.pid);
  const discovery = await client.listTools();
  assert.deepEqual(discovery.tools.map(item => item.name), TOOL_NAMES);
  const capabilities = await call('runtime_capabilities');
  servicePids = capabilities.engines.map(item => item.pid);
  tab = (await call('tab_list')).tabs[0].tab_id;
  assert.equal(typeof tab, 'string');
  const initial = await call('control_status', { tab_id: tab });
  assert.equal(initial.owner, 'human');
  epoch = initial.epoch;
  await toolbarJourney();
  await formJourney(capabilities.demo_url);
  assert.ok(diagnostics.includes('Native MCP toolbar grant and takeover verified.'));
  assert.ok(diagnostics.includes('Native MCP modal profile actions disabled and guarded.'));
  assert.deepEqual(protocolErrors, []);
  await closeCleanly();
  console.log(JSON.stringify({ sdk: 'official MCP', toolbar: 'grant/takeover/fresh grant', forms: 'passed', cleanup: 'EOF, no signals' }));
}

try { await run(); }
catch (error) {
  console.error(error);
  await client.close().catch(() => {});
  process.exitCode = 1;
} finally { clearTimeout(watchdog); }
