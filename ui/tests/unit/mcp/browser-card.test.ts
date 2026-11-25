/** Exercise the live preview's polling decisions through its host messages and DOM events. */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

const post = vi.fn();
const html = readFileSync(resolve('../server/src/mcp/browser-card.html'), 'utf8');

/** Mount the shipped card in a separate document so each preview owns its visibility and timers. */
function mount() {
  const frame = document.body.appendChild(document.createElement('iframe'));
  const win = frame.contentWindow as Window & typeof globalThis;
  const doc = win.document;
  vi.spyOn(win.parent, 'postMessage').mockImplementation(post);
  doc.body.innerHTML = html;
  let intersect: (visible: boolean) => void = () => {};
  let visible = true;
  Object.defineProperty(doc, 'visibilityState', { get: () => (visible ? 'visible' : 'hidden') });
  Object.assign(win, { setTimeout, clearTimeout, setInterval });
  win.IntersectionObserver = class {
    constructor(callback: (entries: { isIntersecting: boolean }[]) => void) {
      intersect = (isIntersecting) => callback([{ isIntersecting }]);
    }
    observe() {}
  } as unknown as typeof IntersectionObserver;
  win.eval(doc.querySelector('script')!.textContent!);
  const send = (msg: object) =>
    win.dispatchEvent(new win.MessageEvent('message', { source: win.parent, data: { jsonrpc: '2.0', ...msg } }));
  send({ method: 'ui/notifications/tool-result', params: { structuredContent: { browserId: 'cloud' } } });
  return {
    doc,
    send,
    intersect,
    visible(value: boolean) {
      visible = value;
      doc.dispatchEvent(new win.Event('visibilitychange'));
    },
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  post.mockClear();
});

afterEach(() => {
  document.body.replaceChildren();
  vi.clearAllTimers();
  vi.useRealTimers();
});

/** Count only capture calls, excluding initialization and layout notifications. */
const calls = () => post.mock.calls.map(([msg]) => msg).filter((msg) => msg.method === 'tools/call');

it('leaves offscreen previews idle when a person returns to the tab', async () => {
  const cards = [mount(), mount()];
  for (const card of cards) {
    card.intersect(false);
    card.visible(false);
    card.visible(true);
  }
  await vi.advanceTimersByTimeAsync(10000);
  expect(calls()).toHaveLength(0);
  cards[0].intersect(true);
  await vi.advanceTimersByTimeAsync(1);
  expect(calls()).toHaveLength(1);
});

it('pauses after repeated capture failures until the person reconnects', async () => {
  const card = mount();
  card.intersect(true);
  await vi.advanceTimersByTimeAsync(1);
  for (let attempt = 0; attempt < 5; attempt++) {
    card.send({ id: calls()[attempt].id, result: { structuredContent: { state: 'failed' } } });
    await vi.advanceTimersByTimeAsync(2000);
  }
  expect(card.doc.getElementById('badge-text')?.textContent).toBe('Disconnected');
  await vi.advanceTimersByTimeAsync(30000);
  expect(calls()).toHaveLength(5);
  card.doc.getElementById('take')!.click();
  await vi.advanceTimersByTimeAsync(1);
  expect(calls()).toHaveLength(6);
});

it('keeps the same image across identical captures and switches when the page changes', async () => {
  const card = mount();
  card.intersect(true);
  await vi.advanceTimersByTimeAsync(1);
  const frames = ['AAAA', 'AAAA', 'BBBB'];
  for (const [index, data] of frames.entries()) {
    const image = 'data:image/png;base64,' + data;
    card.send({ id: calls()[index].id, result: { structuredContent: { state: 'live', image } } });
    await vi.advanceTimersByTimeAsync(1);
    const loaded = card.doc.querySelector<HTMLImageElement>('img[src="' + image + '"]')!;
    if (index === 1) expect(card.doc.querySelectorAll('img[src]')).toHaveLength(1);
    if (index !== 1) loaded.dispatchEvent(new loaded.ownerDocument.defaultView!.Event('load'));
    expect(loaded.classList.contains('on')).toBe(true);
    await vi.advanceTimersByTimeAsync(2000);
  }
});
