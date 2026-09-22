/**
 * Unit tests for openPage: a second CDP connection attached to the browser's
 * first page, over whatever socket the browser's endpoint opens (a dialled
 * one against a loopback browser, or a relay-shaped one), and closed again
 * whenever the attach does not come back.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { WebSocket } from 'ws';
import { openPage } from '../../../../src/modules/gateway/cdp-page.ts';
import { endpointAt } from '../../../../src/drivers/cdp.ts';
import { fakeCdp, pageBrowser } from '../../support/gateway.ts';

/**
 * A socket shaped like the relay an Oya browser's control socket carries: open
 * from the start, answering each command through `answer`, and never dialled.
 */
class RelaySocket extends EventEmitter {
  /** Open from the start, as openRelay hands it over. */
  readyState: number = WebSocket.OPEN;
  /** Commands it was sent, parsed. */
  sent: any[] = [];
  /** How it answers each command. */
  readonly answer: (method: string) => object;

  /** A relay answering through `answer`. */
  constructor(answer: (method: string) => object) {
    super();
    this.answer = answer;
  }

  /** Answers the command on the next tick, the way the browser would. */
  send(data: string) {
    const { id, method } = JSON.parse(data);
    this.sent.push(method);
    setImmediate(() => this.emit('message', Buffer.from(JSON.stringify({ id, result: this.answer(method) })), false));
  }

  /** Closes, as Relay.close does. */
  close() {
    this.readyState = WebSocket.CLOSED;
    this.emit('close');
  }
}

describe('openPage', () => {
  it("attaches to the browser's first page with a flattened session", async () => {
    const browser = await fakeCdp(pageBrowser());
    const page = await openPage(endpointAt(browser.url));
    assert.equal(page.sessionId, 's-1');
    assert.deepEqual(browser.commands[1], {
      method: 'Target.attachToTarget',
      params: { targetId: 't-1', flatten: true },
      sessionId: undefined,
    });
    page.conn.close();
    await browser.close();
  });

  it('attaches over the endpoint it is handed, which need not be dialled', async () => {
    const relay = new RelaySocket((method) =>
      method === 'Target.getTargets' ? { targetInfos: [{ type: 'page', targetId: 't-9' }] } : { sessionId: 's-9' },
    );
    const page = await openPage({ open: async () => relay });
    assert.equal(page.sessionId, 's-9');
    assert.deepEqual(relay.sent, ['Target.getTargets', 'Target.attachToTarget']);
    page.conn.close();
    assert.equal(relay.readyState, WebSocket.CLOSED);
  });

  it('closes the connection when attaching to the page fails', async () => {
    const failing = await fakeCdp((method) => {
      if (method === 'Target.getTargets') return { targetInfos: [{ type: 'page', targetId: 't-1' }] };
      throw new Error('attach refused');
    });
    await assert.rejects(openPage(endpointAt(failing.url)), /attach refused/);
    for (let i = 0; i < 100 && failing.clients() > 0; i++) await new Promise((r) => setTimeout(r, 5));
    assert.equal(failing.clients(), 0, 'the connection was left open');
    await failing.close();
  });

  it('is null, with the connection closed, when the browser has no page', async () => {
    const browser = await fakeCdp((method) =>
      method === 'Target.getTargets' ? { targetInfos: [{ type: 'worker' }] } : {},
    );
    assert.equal(await openPage(endpointAt(browser.url)), null);
    assert.equal(browser.commands.length, 1);
    await browser.close();
  });

  it('treats a browser that reports no targets as having no page', async () => {
    const browser = await fakeCdp(() => ({}));
    assert.equal(await openPage(endpointAt(browser.url)), null);
    await browser.close();
  });
});
