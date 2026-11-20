/**
 * The MCP port over real HTTP on this machine: only an agent with the token gets
 * in, a web page or another host name never does, and each session drives the
 * browser as its own agent, paused like any other in Human Mode.
 */
import { describe, it, before, after, mock } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';

process.env.OYA_MCP_PORT = String(40_000 + Math.floor(Math.random() * 20_000));
const PORT = Number(process.env.OYA_MCP_PORT);
const { LocalMcp } = await import('../../../../src/main/local-mcp/server.ts');

/** A keychain that seals reversibly. */
const keychain = {
  isEncryptionAvailable: () => true,
  getSelectedStorageBackend: () => 'keychain',
  encryptString: (text: string) => Buffer.from(text),
  decryptString: (data: Buffer) => data.toString(),
};

describe('LocalMcp', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-mcp-'));
  const ran: unknown[][] = [];
  let paused = false;
  const deps: any = {
    electron: { app: { getPath: () => dir }, safeStorage: keychain },
    control: {
      beginLocalCommand: async () => {
        if (paused) throw new Error('Automation paused for human control');
        return () => {};
      },
    },
    commands: {
      runLocal: async (...args: unknown[]) => (ran.push(args), { ok: true, data: { page: 'p' }, error: null }),
    },
  };
  const mcp = new LocalMcp(deps);
  before(() => mcp.start());
  after(() => (mcp.stop(), fs.rmSync(dir, { recursive: true, force: true })));

  /** Posts `body` to the port with `headers` (the token unless told otherwise). */
  const post = (body: object, headers: Record<string, string> = {}) =>
    fetch(`http://127.0.0.1:${PORT}/mcp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${mcp.token}`, ...headers },
      body: JSON.stringify(body),
    });
  const init = { jsonrpc: '2.0', id: 1, method: 'initialize', params: {} };

  it('keeps its token across restarts, sealed in the app data folder', () => {
    assert.match(mcp.token, /^oya_local_/);
    assert.equal(new LocalMcp(deps)['secret'], '');
    assert.ok(fs.existsSync(path.join(dir, 'local-mcp-token.enc')));
  });

  it('refuses a request without the token', async () => {
    assert.equal((await post(init, { Authorization: 'Bearer nope' })).status, 401);
  });

  it('refuses a web page, which always sends an Origin, even with the token', async () => {
    assert.equal((await post(init, { Origin: 'https://evil.test' })).status, 403);
  });

  it('refuses another host name, so a rebound DNS name cannot reach it', async () => {
    const status = await new Promise<number>((resolve) => {
      const req = http.request(
        {
          host: '127.0.0.1',
          port: PORT,
          path: '/mcp',
          method: 'POST',
          headers: { Host: 'evil.test', Authorization: `Bearer ${mcp.token}` },
        },
        (res) => (res.resume(), resolve(res.statusCode!)),
      );
      req.end('{}');
    });
    assert.equal(status, 403);
  });

  it('starts a session on initialize and runs each tool call as that session’s own agent', async () => {
    const opened = await post(init);
    const session = opened.headers.get('mcp-session-id')!;
    assert.equal(opened.status, 200);
    const call = { jsonrpc: '2.0', id: 2, method: 'tools/call', params: { name: 'open_tab', arguments: {} } };
    const reply: any = await (await post(call, { 'Mcp-Session-Id': session })).json();
    assert.equal(reply.result.content[0].text, 'p');
    assert.equal(ran.at(-1)![1], `mcp-${session}`);
    assert.equal(ran.at(-1)![2], 'open_tab');
  });

  it('tells the agent to wait while the person holds the browser in Human Mode', async () => {
    paused = true;
    const call = { jsonrpc: '2.0', id: 3, method: 'tools/call', params: { name: 'list_tabs', arguments: {} } };
    const reply: any = await (await post(call)).json();
    paused = false;
    assert.equal(reply.result.isError, true);
    assert.match(reply.result.content[0].text, /OyaAI Mode/);
  });

  it('never offers its token when another program holds the port', async () => {
    const second = new LocalMcp(deps);
    await second.start(); // the first one already holds the port
    assert.equal(second.token, '', 'agents must not be configured to send the token elsewhere');
    second.stop();
  });

  it('keeps control held until a timed-out command really ends', async () => {
    let finished = 0;
    let end: (r: object) => void = () => {};
    const slow: any = {
      ...deps,
      control: { beginLocalCommand: async () => () => finished++ },
      commands: { runLocal: () => new Promise((resolve) => (end = resolve)) },
    };
    mock.timers.enable({ apis: ['setTimeout'] });
    const reply = new LocalMcp(slow)['run']('s', 'click', {});
    await new Promise((resolve) => setImmediate(resolve));
    mock.timers.tick(120_000);
    assert.match((await reply).error, /did not answer in time/);
    assert.equal(finished, 0, 'still running, so still held');
    mock.timers.reset();
    end({ ok: true, data: null, error: null });
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(finished, 1);
  });

  it('answers a notification with no body', async () => {
    assert.equal((await post({ jsonrpc: '2.0', method: 'notifications/initialized' })).status, 202);
  });

  it('releases the session lane when the client ends its session with DELETE /mcp', async () => {
    const released: string[] = [];
    const { currentLane } = await import('../../../../src/main/control/agent-tab.ts');
    deps.agentTab = { release: () => released.push(currentLane()) };
    const opened = await post(init);
    const session = opened.headers.get('mcp-session-id')!;
    const ended = await fetch(`http://127.0.0.1:${PORT}/mcp`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${mcp.token}`, 'Mcp-Session-Id': session },
    });
    assert.equal(ended.status, 200);
    assert.deepEqual(released, [`mcp-${session}`]);
  });
});
