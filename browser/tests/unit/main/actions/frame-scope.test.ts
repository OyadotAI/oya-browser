/**
 * Unit tests for cross-origin frames in the page read and in actions: every cross-origin frame
 * at any depth is read under its row with ids in its own range (a new row when its row cannot
 * be told apart), an action on a frame id runs in that frame, its clicks reach the page through
 * the frame's exact owners and are refused under an overlay, and a frame that loaded another
 * document since the read is refused.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  withFrames,
  frameTarget,
  frameDriver,
  slotOf,
  targetOf,
  type FrameLinks,
} from '../../../../src/main/actions/frame-scope.ts';
import { FRAME_ID_STRIDE, FRAME_GONE_ERROR, FRAME_OUTSIDE_ERROR } from '../../../../src/main/actions/constants.ts';

/** A fake native frame: its address, its parent, and whether it is gone. */
interface Frame {
  /** Its address. */
  url: string;
  /** The frame that holds it; null for the page. */
  parent: Frame | null;
  /** Whether it is gone. */
  detached: boolean;
}

/** The page's frame, on https://a.test/. */
const page = (): Frame => ({ url: 'https://a.test/', parent: null, detached: false });

/** A frame on `url` inside `parent`. */
const frame = (url: string, parent: Frame): Frame => ({ url, parent, detached: false });

/** A view whose page frame is `main`. */
const view = (main: Frame) => ({ webContents: { mainFrame: main } });

/** A read of some text and one iframe row per target, with one element of its own. */
const read = (text: string, ...targets: string[]) => ({
  ok: true,
  data: {
    blocks: [{ kind: 'text', text }, ...targets.map((target) => ({ kind: 'iframe', target }))],
    elements: [{ id: 1, type: 'link', text }],
  },
});

/** How the fake links answer: each frame's generation, and whether every owner is covered. */
interface LinkOptions {
  /** Each frame's document generation ('g1' when left out). */
  generations?: Map<Frame, string>;
  /** Whether an overlay covers the frame's owners. */
  covered?: boolean;
}

/** Frame links over `frames`: each frame's generation is in `generations` (default 'g1'); toTop records or refuses. */
function links(frames: Frame[], opts: LinkOptions = {}) {
  const lifted: number[][] = [];
  const value: FrameLinks = {
    frames: () => frames as never,
    generation: async (_v, f) => opts.generations?.get(f as never) ?? 'g1',
    toTop: async (_v, _f, x, y) => {
      if (opts.covered) throw new Error('Frame is covered at the requested point.');
      lifted.push([x, y]);
      return { x: x + 100, y: y + 200 };
    },
  };
  return Object.assign(value, { lifted });
}

/** A driver whose scripts answer `answer(expression, frame)`, with `frameLinks`; its mouse records native clicks. */
function driver(answer: (expression: string, frame?: Frame) => unknown, frameLinks: FrameLinks) {
  const clicks: number[][] = [];
  const deps = { worldEval: async (_v: unknown, e: string, f?: Frame) => answer(e, f), frameLinks };
  const mouse = { click: async (_v: unknown, x: number, y: number) => void clicks.push([x, y]) };
  return { deps, mouse, keyboard: {}, clicks };
}

/** Each frame read as `reads.get(frame)`. */
const reading = (reads: Map<Frame, unknown>) => (_e: string, f?: Frame) => (f ? reads.get(f) : null);

/** The blocks of a read as [kind, id or target]. */
const shape = (raw: ReturnType<typeof read>) =>
  raw.data.blocks.map((b) => [
    b.kind,
    (b as Record<string, unknown>).id ?? (b as Record<string, unknown>).target ?? b.text,
  ]);

describe('withFrames', () => {
  it('reads a cross-origin frame in place of its row, with ids from its own range', async () => {
    const main = page();
    const b = frame('https://b.test/form', main);
    const v = view(main);
    const raw = read('Portal', 'https://b.test/form?x=1');
    await withFrames(driver(reading(new Map([[b, read('Name')]])), links([b])) as never, v as never, raw, 'analyze');
    const offset = slotOf(v as never, b as never) * FRAME_ID_STRIDE;
    assert.deepEqual(shape(raw), [
      ['text', 'Portal'],
      ['iframe', 'https://b.test/form?x=1'], // kept as the frame's heading, so its text is not taken for the page's
      ['text', 'Name'],
    ]);
    assert.deepEqual(
      raw.data.elements.map((e) => e.id),
      [1, offset + 1],
    );
  });

  it('reads a frame nested inside another cross-origin frame, under its row in that frame’s read', async () => {
    const main = page();
    const b = frame('https://b.test/', main);
    const c = frame('https://c.test/card', b);
    const v = view(main);
    const raw = read('Shop', 'https://b.test/');
    const reads = new Map<Frame, unknown>([
      [b, read('Checkout', 'https://c.test/card')],
      [c, read('Card number')],
    ]);
    await withFrames(driver(reading(reads), links([b, c])) as never, v as never, raw, 'analyze');
    assert.deepEqual(
      shape(raw).map(([kind, what]) => (kind === 'iframe' ? `iframe ${what}` : what)),
      ['Shop', 'iframe https://b.test/', 'Checkout', 'iframe https://c.test/card', 'Card number'],
    );
    const cOffset = slotOf(v as never, c as never) * FRAME_ID_STRIDE;
    assert.ok(raw.data.elements.some((e) => e.id === cOffset + 1 && e.text === 'Card number'));
  });

  it('reads two frames from the same site, each under a row of its own and in its own id range', async () => {
    const main = page();
    const one = frame('https://b.test/one', main);
    const two = frame('https://b.test/two', main);
    const v = view(main);
    const raw = read('Portal', 'https://b.test/one', 'https://b.test/two');
    const reads = new Map<Frame, unknown>([
      [one, read('First')],
      [two, read('Second')],
    ]);
    await withFrames(driver(reading(reads), links([one, two])) as never, v as never, raw, 'analyze');
    const texts = raw.data.elements.map((e) => [e.id, e.text]);
    assert.deepEqual(texts.slice(1), [
      [slotOf(v as never, one as never) * FRAME_ID_STRIDE + 1, 'First'],
      [slotOf(v as never, two as never) * FRAME_ID_STRIDE + 1, 'Second'],
    ]);
    assert.ok(shape(raw).some(([k, t]) => k === 'iframe' && t === 'https://b.test/one'));
  });

  it('does not read a same-origin frame inside a cross-origin one on its own: its parent’s read has it', async () => {
    const main = page();
    const b = frame('https://b.test/', main);
    const inner = frame('https://b.test/inner', b);
    const answer = (_e: string, f?: Frame) => (f === inner ? assert.fail('read inside b') : f ? read('B') : null);
    await withFrames(
      driver(answer, links([b, inner])) as never,
      view(main) as never,
      read('A', 'https://b.test/'),
      'analyze',
    );
  });

  it('does not read a frame that loaded another document while it was being read', async () => {
    const main = page();
    const b = frame('https://b.test/', main);
    const raw = read('Portal', 'https://b.test/');
    let calls = 0;
    const l = { ...links([b]), generation: async () => (calls++ === 0 ? 'g1' : 'g2') } as FrameLinks;
    await withFrames(driver(reading(new Map([[b, read('X')]])), l) as never, view(main) as never, raw, 'analyze');
    assert.match(String(raw.data.blocks[1].target), /changed while being read/);
    assert.equal(raw.data.elements.length, 1, 'none of its elements were added');
  });

  it('keeps a row and says why when its frame does not answer', async () => {
    const main = page();
    const raw = read('Portal', 'https://b.test/');
    const silent = driver(
      () => Promise.reject(new Error('The frame did not answer')),
      links([frame('https://b.test/', main)]),
    );
    await withFrames(silent as never, view(main) as never, raw, 'analyze');
    assert.match(String(raw.data.blocks[1].target), /not readable: The frame did not answer/);
  });

  it('counts the frame’s elements in the read’s header', async () => {
    const main = page();
    const b = frame('https://b.test/', main);
    const raw = read('Portal', 'https://b.test/');
    const data = raw.data as Record<string, unknown>;
    data.facts = { elements: '1 total, 1 visible' };
    await withFrames(
      driver(reading(new Map([[b, read('X')]])), links([b])) as never,
      view(main) as never,
      raw,
      'analyze',
    );
    assert.equal((data.facts as Record<string, string>).elements, '2 total, 0 visible');
  });

  it('leaves out a frame element whose id would reach the next frame’s range', async () => {
    const main = page();
    const b = frame('https://b.test/', main);
    const huge = { ok: true, data: { blocks: [], elements: [{ id: FRAME_ID_STRIDE, type: 'button', text: 'X' }] } };
    const raw = read('Portal', 'https://b.test/');
    await withFrames(driver(() => huge, links([b])) as never, view(main) as never, raw, 'analyze');
    assert.deepEqual(
      raw.data.elements.map((e) => e.id),
      [1],
    );
  });

  it('never reads a CAPTCHA challenge frame, or anything inside one', async () => {
    const main = page();
    const challenge = frame('https://www.google.com/recaptcha/api2/anchor', main);
    const inside = frame('https://d.test/', challenge);
    const raw = read('Login', 'https://www.google.com/recaptcha/api2/anchor');
    const d = driver(() => assert.fail('challenge frames are not read'), links([challenge, inside]));
    await withFrames(d as never, view(main) as never, raw, 'analyze');
    assert.equal(raw.data.blocks.length, 2);
  });

  it('leaves the page’s own read as it was when the frame tree is too large to walk', async () => {
    const raw = read('Portal', 'https://b.test/');
    const before = structuredClone(raw);
    const tooMany = { ...links([]), frames: () => assert.fail('Frame inventory exceeds 128 documents') } as FrameLinks;
    await withFrames(
      driver(() => assert.fail('nothing read'), tooMany) as never,
      view(page()) as never,
      raw,
      'analyze',
    );
    assert.deepEqual(raw, before);
  });

  it('leaves a page with no cross-origin frames as it was', async () => {
    const raw = read('Portal');
    const before = structuredClone(raw);
    await withFrames(
      driver(() => assert.fail('nothing to read'), links([])) as never,
      view(page()) as never,
      raw,
      'analyze',
    );
    assert.deepEqual(raw, before);
  });
});

describe('frameTarget', () => {
  it('leaves the page’s own elements alone', () => {
    assert.equal(frameTarget(view(page()) as never, '[data-ac-id="12"]'), null);
  });

  it('names the frame and the element’s own id for an id in a frame’s range', async () => {
    const main = page();
    const b = frame('https://b.test/', main);
    const v = view(main);
    await withFrames(
      driver(reading(new Map([[b, read('X')]])), links([b])) as never,
      v as never,
      read('P', 'https://b.test/'),
      'analyze',
    );
    const slot = slotOf(v as never, b as never);
    assert.deepEqual(frameTarget(v as never, `[data-ac-id="${slot * FRAME_ID_STRIDE + 7}"]`), {
      frame: b,
      selector: '[data-ac-id="7"]',
    });
  });

  it('refuses an id whose frame navigated since the read, though the frame object is the same', async () => {
    const main = page();
    const b = frame('https://b.test/', main);
    const v = view(main);
    await withFrames(
      driver(reading(new Map([[b, read('X')]])), links([b])) as never,
      v as never,
      read('P', 'https://b.test/'),
      'analyze',
    );
    const id = slotOf(v as never, b as never) * FRAME_ID_STRIDE + 1;
    b.url = 'https://b.test/next';
    assert.equal(frameTarget(v as never, `[data-ac-id="${id}"]`), FRAME_GONE_ERROR);
  });

  it('refuses an id whose frame has gone since the read', () => {
    const b = frame('https://b.test/', page());
    const v = view(page());
    const id = slotOf(v as never, b as never) * FRAME_ID_STRIDE + 1;
    b.detached = true;
    assert.equal(frameTarget(v as never, `[data-ac-id="${id}"]`), FRAME_GONE_ERROR);
  });
});

/** The parts of a frame driver the tests call. */
interface Scoped {
  /** Its dependencies. */
  deps: {
    /** Runs a script. */
    worldEval(v: unknown, e: string): Promise<unknown>;
  };
  /** Its mouse. */
  mouse: {
    /** Clicks at a frame point. */
    click(v: unknown, x: number, y: number): Promise<void>;
  };
  /** Its keyboard. */
  keyboard: {
    /** Types text. */
    type(v: unknown, t: string): Promise<void>;
    /** Presses a key. */
    press(v: unknown, k: string): Promise<void>;
  };
}

describe('frameDriver', () => {
  /** A frame b, a driver whose frame scripts are recorded and answer true, and its links. */
  function setup(opts: Parameters<typeof links>[1] = {}) {
    const main = page();
    const b = frame('https://b.test/', main);
    const inFrame: string[] = [];
    const l = links([b], opts);
    const d = driver((e, f) => (f ? (inFrame.push(e), true) : undefined), l);
    return { b, v: view(main), d, l, inFrame };
  }

  /** The command driver for frame `b`. */
  const scope = async (s: ReturnType<typeof setup>) =>
    (await frameDriver(s.d as never, s.v as never, s.b as never)) as never as Scoped;

  it('runs the command’s scripts in the frame', async () => {
    const s = setup();
    await (await scope(s)).deps.worldEval(null, 'probe');
    assert.ok(s.inFrame.some((e) => e.includes('probe')));
  });

  it('reaches the page through the frame’s owners, then presses the element inside the frame, never with a native click', async () => {
    const s = setup();
    await (await scope(s)).mouse.click(null, 10, 20);
    assert.deepEqual(s.l.lifted, [[10, 20]]);
    assert.ok(s.inFrame.some((e) => e.includes('elementFromPoint') && e.includes('10, 20')));
    assert.deepEqual(s.d.clicks, []);
  });

  it('types and presses keys inside the frame, since native keys never reach it', async () => {
    const s = setup();
    const k = (await scope(s)).keyboard;
    await k.type(null, 'Ada');
    await k.press(null, 'Enter');
    assert.ok(s.inFrame.some((e) => e.includes('insertText') && e.includes('"Ada"')));
    assert.ok(s.inFrame.some((e) => e.includes('requestSubmit') && e.includes('"Enter"')));
  });

  it('tells the agent when a key cannot be honoured in the frame, instead of reporting success', async () => {
    const main = page();
    const b = frame('https://b.test/', main);
    const d = driver((e, f) => (f ? !e.includes('"ArrowDown"') : true), links([b]));
    const s = (await frameDriver(d as never, view(main) as never, b as never)) as never as Scoped;
    await assert.rejects(s.keyboard.press(null, 'ArrowDown'), /cannot be pressed inside another site/);
    await s.keyboard.press(null, 'Tab');
  });

  it('refuses a point that is not a number, whatever the frame answered', async () => {
    const s = setup();
    await assert.rejects((await scope(s)).mouse.click(null, '1); evil(' as never, 20), {
      message: FRAME_OUTSIDE_ERROR,
    });
    assert.deepEqual(s.inFrame, []);
  });

  it('refuses a click an overlay drawn over the frame, or over any frame holding it, would take', async () => {
    const s = setup({ covered: true });
    await assert.rejects((await scope(s)).mouse.click(null, 10, 20), /covered/);
    assert.deepEqual(s.inFrame, []);
  });

  it('refuses a frame that loaded another document since the read, even at the same address', async () => {
    const generations = new Map<Frame, string>();
    const s = setup({ generations });
    await withFrames(
      driver(reading(new Map([[s.b, read('X')]])), s.l) as never,
      s.v as never,
      read('P', 'https://b.test/'),
      'analyze',
    );
    generations.set(s.b, 'g2');
    await assert.rejects(frameDriver(s.d as never, s.v as never, s.b as never), { message: FRAME_GONE_ERROR });
  });

  it('sends a key to the focused frame even after it loaded its next page, since the key names no element from a read', async () => {
    const generations = new Map<Frame, string>();
    const s = setup({ generations });
    await withFrames(
      driver(reading(new Map([[s.b, read('X')]])), s.l) as never,
      s.v as never,
      read('P', 'https://b.test/'),
      'analyze',
    );
    generations.set(s.b, 'g2');
    await frameDriver(s.d as never, s.v as never, s.b as never, false);
  });

  it('refuses when the frame is gone', async () => {
    const s = setup();
    s.b.detached = true;
    await assert.rejects(frameDriver(s.d as never, s.v as never, s.b as never), { message: FRAME_GONE_ERROR });
  });
});

describe('targetOf', () => {
  /** Each document's focus state: the page and each frame by its address, as the focus script answers. */
  const focus = (states: Record<string, object>) => (e: string, f?: Frame) =>
    e.includes('activeElement') ? (states[f ? f.url : 'page'] ?? { leaf: false, origin: '' }) : null;

  it('sends a key with no element down the focus path to the nested frame whose field has it, with the window in the background', async () => {
    const main = page();
    const b = frame('https://b.test/', main);
    const c = frame('https://c.test/', b);
    const states = {
      page: { leaf: false, origin: 'https://b.test' },
      'https://b.test/': { leaf: false, origin: 'https://c.test' },
      'https://c.test/': { leaf: true, origin: '' },
    };
    const target = await targetOf(driver(focus(states), links([b, c])) as never, view(main) as never, 'press_key', {
      key: 'Enter',
    } as never);
    assert.equal((target as never as Record<'frame', Frame>).frame, c);
  });

  it('ignores a frame that remembers a focused field the page has since moved away from', async () => {
    const main = page();
    const b = frame('https://b.test/', main);
    const states = { page: { leaf: true, origin: '' }, 'https://b.test/': { leaf: true, origin: '' } };
    const d = driver(focus(states), links([b]));
    assert.equal(await targetOf(d as never, view(main) as never, 'press_key', { key: 'Enter' } as never), null);
  });

  it('leaves a focused same-site frame to native keys, which reach it', async () => {
    const main = page();
    const same = frame('https://a.test/inner', main);
    const states = {
      page: { leaf: false, origin: 'https://a.test' },
      'https://a.test/inner': { leaf: true, origin: '' },
    };
    const d = driver(focus(states), links([same]));
    assert.equal(await targetOf(d as never, view(main) as never, 'press_key', { key: 'Enter' } as never), null);
  });
});
