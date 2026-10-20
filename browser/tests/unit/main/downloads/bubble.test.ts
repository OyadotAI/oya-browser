/**
 * Unit tests for the download chip's feed from main: a window hears only the
 * downloads that started from its own pages, in the profile on screen, never
 * where a file was saved, and nothing kept from an earlier launch.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { DownloadBubble } from '../../../../src/main/downloads/index.ts';

/** A window: its shell records what it is sent; `focused` and `alive` say how it stands. */
function window(name: string) {
  const sent: any[] = [];
  const win = Object.assign(new EventEmitter(), { focused: true, isFocused: () => win.focused });
  const shell = { window: win, open: true, alive: () => shell.open, send: (_c: string, s: unknown) => sent.push(s) };
  return { name, shell, sent, win };
}

/** A download entry as the list gives it (never with its path). */
const entry = (id: string, state = 'progressing', over: Record<string, unknown> = {}) => ({
  ...{ id, url: `https://files.test/${id}`, title: `${id}.pdf`, time: 1, state, received: 10, total: 20 },
  ...over,
});

/** A chip feed over two windows, A and B, and a download list the test fills. */
function setup() {
  const [a, b] = [window('a'), window('b')];
  const pages = new Map<unknown, unknown>([
    ['page-a', a],
    ['page-b', b],
  ]);
  let list: any[] = [];
  const windows = { current: a, ownerOfContents: (c: unknown) => pages.get(c) };
  const bubble = new DownloadBubble({ windows, downloads: { list: () => list } } as any);
  return { a, b, bubble, setList: (l: any[]) => void (list = l) };
}

it('tells only the window the download came from', () => {
  const { a, b, bubble, setList } = setup();
  setList([entry('d1')]);
  bubble.began('d1', 'page-b' as any);
  assert.equal(a.sent.length, 0);
  assert.equal(b.sent.at(-1).latest.id, 'd1');
  bubble.announce();
  assert.equal(a.sent.length, 0, 'an update is not a broadcast');
});

it('a download from a page in no tab (a popup window, a tab already closed) goes to the window in front', () => {
  const { a, bubble, setList } = setup();
  setList([entry('d1')]);
  bubble.began('d1', undefined);
  assert.equal(a.sent.at(-1).latest.id, 'd1');
});

it('never shows an entry kept from an earlier launch', () => {
  const { a, bubble, setList } = setup();
  setList([entry('old', 'interrupted'), entry('d1')]);
  bubble.began('d1', 'page-a' as any);
  setList([entry('d1', 'completed'), entry('old', 'interrupted')]);
  bubble.announce();
  assert.deepEqual(
    a.sent.map((s) => s.latest.id),
    ['d1', 'd1'],
  );
});

it("never shows another profile's download, and a switch clears the chip", () => {
  const { a, bubble, setList } = setup();
  setList([entry('d1', 'completed')]);
  bubble.began('d1', 'page-a' as any);
  setList([]);
  bubble.announce();
  assert.equal(a.sent.at(-1).latest, null);
  bubble.announce();
  assert.equal(a.sent.length, 2, 'then the window is forgotten');
});

it('a download that ends at once is told running, then completed', () => {
  const { a, bubble, setList } = setup();
  setList([entry('d1')]);
  bubble.began('d1', 'page-a' as any);
  setList([entry('d1', 'completed', { received: 20 })]);
  bubble.announce();
  assert.deepEqual(
    a.sent.map((s) => [s.latest.state, s.running]),
    [
      ['progressing', 1],
      ['completed', 0],
    ],
  );
});

it("watches a window's focus once, however often it is forgotten and shown again", () => {
  const { a, bubble, setList } = setup();
  for (let i = 0; i < 3; i++) {
    setList([entry(`d${i}`, 'completed')]);
    bubble.began(`d${i}`, 'page-a' as any);
    setList([]);
    bubble.announce();
  }
  assert.equal(a.win.listenerCount('focus'), 1);
  const before = a.sent.length;
  a.win.emit('focus');
  assert.equal(a.sent.length, before, 'a forgotten window is not told on focus');
});

it('never sends where a file was saved', () => {
  const { a, bubble, setList } = setup();
  setList([entry('d1')]);
  bubble.began('d1', 'page-a' as any);
  assert.doesNotMatch(JSON.stringify(a.sent), /path|partition|Downloads\//);
});

it('totals what is running, and has no total when any size is unknown', () => {
  const { a, bubble, setList } = setup();
  setList([entry('d2'), entry('d1')]);
  bubble.began('d1', 'page-a' as any);
  bubble.began('d2', 'page-a' as any);
  assert.deepEqual([a.sent.at(-1).running, a.sent.at(-1).received, a.sent.at(-1).total], [2, 20, 40]);
  setList([entry('d2', 'progressing', { total: 0 }), entry('d1')]);
  bubble.announce();
  assert.equal(a.sent.at(-1).total, null);
});

it('says whether the window is focused, and tells it again when that changes', () => {
  const { a, bubble, setList } = setup();
  setList([entry('d1', 'completed')]);
  a.win.focused = false;
  bubble.began('d1', 'page-a' as any);
  assert.equal(a.sent.at(-1).focused, false);
  a.win.focused = true;
  a.win.emit('focus');
  assert.equal(a.sent.at(-1).focused, true);
});

it('skips and forgets a closed window', () => {
  const { a, bubble, setList } = setup();
  setList([entry('d1')]);
  bubble.began('d1', 'page-a' as any);
  a.shell.open = false;
  bubble.announce();
  bubble.announce();
  assert.equal(a.sent.length, 1);
});
