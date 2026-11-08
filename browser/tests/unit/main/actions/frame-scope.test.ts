/**
 * Unit tests for cross-origin iframes in the page read and in actions: a matched frame's
 * read replaces its row with ids in its own range, an uncertain match stays a row with a
 * reason, an action on a frame id runs in that frame, and its points are moved into the
 * page and refused outside the frame or under an overlay.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { withFrames, frameTarget, frameDriver, slotOf } from '../../../../src/main/actions/frame-scope.ts';
import { FRAME_ID_STRIDE, FRAME_GONE_ERROR, FRAME_OUTSIDE_ERROR } from '../../../../src/main/actions/constants.ts';

/** A child frame on `url`. */
const frame = (url: string) => ({ url, detached: false });

/** A view of https://a.test/ with `frames` as its direct children. */
const view = (frames: object[]) => ({ webContents: { mainFrame: { url: 'https://a.test/', frames } } });

/** The page's own read: some text, then one iframe row per target. */
const pageRead = (...targets: string[]) => ({
  ok: true,
  data: {
    blocks: [{ kind: 'text', text: 'Portal' }, ...targets.map((target) => ({ kind: 'iframe', target }))],
    elements: [{ id: 1, type: 'link', text: 'Home' }],
  },
});

/** A frame's own read: one input and one button, numbered from 1. */
const frameRead = {
  ok: true,
  data: {
    blocks: [
      { kind: 'element', id: 1, text: 'Name' },
      { kind: 'element', id: 2, text: 'Create' },
    ],
    elements: [
      { id: 1, type: 'input', text: 'Name' },
      { id: 2, type: 'button', text: 'Create' },
    ],
  },
};

/** The fake driver, as tests use it. */
interface FakeDriver {
  /** Its dependencies, worldEval among them. */
  deps: never;
  /** Its mouse. */
  mouse: never;
  /** Where its mouse clicked. */
  clicks: number[][];
}

/** A driver whose scripts answer `answer(expression, frame)` and whose mouse records where it clicked. */
function driver(answer: (expression: string, frame?: object) => unknown) {
  const clicks: number[][] = [];
  const deps = {
    /** Answers a script, in the page or in frame `f`. */
    worldEval: async (_v: unknown, e: string, f?: object) => answer(e, f),
  };
  const mouse = {
    /** Records a click at page point (x, y). */
    click: async (_v: unknown, x: number, y: number) => void clicks.push([x, y]),
  };
  return { deps, mouse, keyboard: {}, clicks } as never as FakeDriver;
}

describe('withFrames', () => {
  it('reads a cross-origin frame in place of its row, with ids from its own range', async () => {
    const b = frame('https://b.test/form');
    const v = view([b]);
    const raw = pageRead('https://b.test/form?x=1');
    await withFrames(driver((_e, f) => (f === b ? frameRead : null)) as never, v as never, raw, 'analyze');
    const offset = slotOf(v as never, b as never) * FRAME_ID_STRIDE;
    assert.deepEqual(
      raw.data.blocks.map((x) => [x.kind, (x as Record<string, unknown>).id]),
      [
        ['text', undefined],
        ['iframe', undefined], // kept as the frame's heading, so its text is not taken for the page's
        ['element', offset + 1],
        ['element', offset + 2],
      ],
    );
    assert.deepEqual(
      raw.data.elements.map((e) => e.id),
      [1, offset + 1, offset + 2],
    );
  });

  it('keeps a row and says why when two frames come from the same site, so no read goes to the wrong one', async () => {
    const raw = pageRead('https://b.test/one', 'https://b.test/two');
    const v = view([frame('https://b.test/one'), frame('https://b.test/two')]);
    await withFrames(driver(() => assert.fail('no frame is read')) as never, v as never, raw, 'analyze');
    assert.match(String(raw.data.blocks[1].target), /several frames from this site/);
  });

  it('keeps a row and says why when its frame does not answer', async () => {
    const raw = pageRead('https://b.test/');
    const silent = driver(() => Promise.reject(new Error('The frame did not answer')));
    await withFrames(silent as never, view([frame('https://b.test/')]) as never, raw, 'analyze');
    assert.match(String(raw.data.blocks[1].target), /not readable: The frame did not answer/);
  });

  it('counts the frame’s elements in the read’s header', async () => {
    const b = frame('https://b.test/');
    const raw = pageRead('https://b.test/');
    const data = raw.data as Record<string, unknown>;
    data.facts = { elements: '1 total, 1 visible' };
    await withFrames(driver(() => frameRead) as never, view([b]) as never, raw, 'analyze');
    assert.equal((data.facts as Record<string, string>).elements, '3 total, 0 visible');
  });

  it('leaves out a frame element whose id would reach the next frame’s range', async () => {
    const b = frame('https://b.test/');
    const huge = { ok: true, data: { blocks: [], elements: [{ id: FRAME_ID_STRIDE, type: 'button', text: 'X' }] } };
    const raw = pageRead('https://b.test/');
    await withFrames(driver(() => huge) as never, view([b]) as never, raw, 'analyze');
    assert.deepEqual(
      raw.data.elements.map((e) => e.id),
      [1],
    );
  });

  it('never reads a CAPTCHA challenge frame, which watches for exactly that', async () => {
    const raw = pageRead('https://www.google.com/recaptcha/api2/anchor');
    const v = view([frame('https://www.google.com/recaptcha/api2/anchor')]);
    await withFrames(driver(() => assert.fail('challenge frames are not read')) as never, v as never, raw, 'analyze');
    assert.equal(raw.data.blocks.length, 2);
  });

  it('leaves a page with no cross-origin frames as it was', async () => {
    const raw = pageRead();
    const before = structuredClone(raw);
    await withFrames(driver(() => assert.fail('nothing to read')) as never, view([]) as never, raw, 'analyze');
    assert.deepEqual(raw, before);
  });
});

describe('frameTarget', () => {
  it('leaves the page’s own elements alone', () => {
    assert.equal(frameTarget(view([]) as never, '[data-ac-id="12"]'), null);
  });

  it('names the frame and the element’s own id for an id in a frame’s range', async () => {
    const b = frame('https://b.test/');
    const v = view([b]);
    await withFrames(driver(() => frameRead) as never, v as never, pageRead('https://b.test/'), 'analyze');
    const slot = slotOf(v as never, b as never);
    const target = frameTarget(v as never, `[data-ac-id="${slot * FRAME_ID_STRIDE + 7}"]`);
    assert.deepEqual(target, { frame: b, selector: '[data-ac-id="7"]' });
  });

  it('refuses an id whose frame navigated since the read, though the frame object is the same', async () => {
    const b = frame('https://b.test/');
    const v = view([b]);
    await withFrames(driver(() => frameRead) as never, v as never, pageRead('https://b.test/'), 'analyze');
    const id = slotOf(v as never, b as never) * FRAME_ID_STRIDE + 1;
    b.url = 'https://b.test/next';
    assert.equal(frameTarget(v as never, `[data-ac-id="${id}"]`), FRAME_GONE_ERROR);
  });

  it('refuses an id whose frame has gone since the read', () => {
    const b = frame('https://b.test/');
    const v = view([b]);
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
  /**
   * A driver whose page reports the frame's box at (100, 200), 300 by 150, and whether a page
   * point hits the frame; scripts sent to the frame are recorded, and a click there answers true.
   */
  function boxed(hits = true) {
    const inFrame: string[] = [];
    const d = driver((e, f) => {
      if (f) {
        inFrame.push(e);
        return true;
      }
      if (e.includes('getBoundingClientRect')) return { ok: true, x: 100, y: 200, w: 300, h: 150 };
      return e.includes('elementFromPoint') ? hits : undefined;
    });
    return Object.assign(d, { inFrame });
  }

  /** The command driver for frame `b` of `d`. */
  const scope = async (d: ReturnType<typeof boxed>, b = frame('https://b.test/')) =>
    (await frameDriver(d as never, view([b]) as never, b as never)) as never as Scoped;

  it('runs the command’s scripts in the frame', async () => {
    const d = boxed();
    await (await scope(d)).deps.worldEval(null, 'probe');
    assert.ok(d.inFrame.some((e) => e.includes('probe')));
  });

  it('presses the element inside the frame at the read’s point, never with a native click that would miss it', async () => {
    const d = boxed();
    await (await scope(d)).mouse.click(null, 10, 20);
    assert.ok(d.inFrame.some((e) => e.includes('elementFromPoint') && e.includes('10, 20')));
    assert.deepEqual(d.clicks, []);
  });

  it('types and presses keys inside the frame, since native keys never reach it', async () => {
    const d = boxed();
    const s = await scope(d);
    await s.keyboard.type(null, 'Ada');
    await s.keyboard.press(null, 'Enter');
    assert.ok(d.inFrame.some((e) => e.includes('insertText') && e.includes('"Ada"')));
    assert.ok(d.inFrame.some((e) => e.includes('requestSubmit') && e.includes('"Enter"')));
  });

  it('tells the agent when a key cannot be honoured in the frame, instead of reporting success', async () => {
    const d = driver((e, f) => {
      if (f) return !e.includes('"ArrowDown"');
      return e.includes('getBoundingClientRect') ? { ok: true, x: 0, y: 0, w: 10, h: 10 } : true;
    });
    const b = frame('https://b.test/');
    const s = (await frameDriver(d as never, view([b]) as never, b as never)) as never as Scoped;
    await assert.rejects(s.keyboard.press(null, 'ArrowDown'), /cannot be pressed inside another site/);
    await s.keyboard.press(null, 'Tab');
  });

  it('refuses a point outside the frame’s box, so a frame cannot steer input elsewhere', async () => {
    const d = boxed();
    await assert.rejects((await scope(d)).mouse.click(null, 400, 20), { message: FRAME_OUTSIDE_ERROR });
    assert.deepEqual(d.inFrame, []);
  });

  it('refuses a point that is not a number, whatever the frame answered', async () => {
    const d = boxed();
    await assert.rejects((await scope(d)).mouse.click(null, '1); evil(' as never, 20), {
      message: FRAME_OUTSIDE_ERROR,
    });
    assert.deepEqual(d.inFrame, []);
  });

  it('refuses a click an overlay drawn over the frame would take', async () => {
    const d = boxed(false);
    await assert.rejects((await scope(d)).mouse.click(null, 10, 20), /covered/);
    assert.deepEqual(d.inFrame, []);
  });

  it('refuses when the frame’s iframe is no longer in the page', async () => {
    const d = driver(() => ({ ok: false }));
    const b = frame('https://b.test/');
    await assert.rejects(frameDriver(d as never, view([b]) as never, b as never), { message: FRAME_GONE_ERROR });
  });
});
