/**
 * Unit tests for src/main/translate: when translation is offered, what a run
 * sends and writes, how it ends (done, failed, stopped, navigated away), and
 * that a tab an agent claims or a recording starts on is restored before the
 * agent or the recorder reads it.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { Translator } from '../../../../src/main/translate/index.ts';
import { batches, failureOf } from '../../../../src/main/translate/translate-run.ts';
import {
  TRANSLATE_BATCH_CHARS,
  TRANSLATE_FIRST_BATCH_CHARS,
  TRANSLATE_KEEP_PAGES,
  TRANSLATE_PARALLEL,
} from '../../../../src/main/translate/constants.ts';

/** A tab's page, answering translation's scripts: `lang` it declares, `texts` it holds. */
function contents(lang: string, texts: string[], partial = false) {
  const page = {
    lang,
    document: 'fixture-document',
    applied: [] as string[][],
    restored: 0,
    restore: Promise.resolve(),
    identify: Promise.resolve(),
  };
  const run = async (code: string) => {
    if (code.includes('return { document: globalThis.__oyaTranslationDocument')) {
      const reply = { document: page.document, lang: page.lang };
      await page.identify;
      return reply;
    }
    if (code.includes('Math.max(globalThis.__oyaTranslationSequence')) return page.restore.then(() => page.restored++);
    if (code.includes('maxChars, batchChars')) return { texts, partial };
    page.applied.push(JSON.parse(code.slice(code.lastIndexOf(', [') + 2, code.lastIndexOf(');'))));
  };
  const webContents = {
    isDestroyed: () => false,
    isLoading: () => true,
    executeJavaScriptInIsolatedWorld: (_world: number, [{ code }]: Array<Record<'code', string>>) => run(code),
  };
  return { page, webContents };
}

/** The server: answers each batch through `answer` (by default, each text upper-cased) and keeps what it was sent. */
function server(
  answer = async (body: any) => Response.json({ texts: body.texts.map((t: string) => t.toUpperCase()) }),
) {
  const sent: any[] = [];
  return {
    sent,
    canCall: () => true,
    postToBrowser: async (_route: string, body: any) => (sent.push(body), answer(body)),
  };
}

/** A Translator over one tab on `url`, with the person reading `osLanguages` and `over` changing any service. */
function setup(page = contents('ja', ['こんにちは']), api = server(), over: Record<string, any> = {}) {
  const tab: any = { id: 1, url: 'https://example.jp/', view: { webContents: page.webContents } };
  const claimed = new Set<number>();
  const deps: any = {
    config: { values: {} },
    governance: { configuration: null },
    recorder: { recording: false },
    agentTab: { humanMay: (id: number) => !claimed.has(id) },
    tabs: { list: [tab] },
    electron: { app: { getPreferredSystemLanguages: () => ['en-US', 'fr'], getLocale: () => 'en-US' } },
    ...over,
  };
  let changes = 0;
  const translator = new Translator(deps, () => changes++, api as any);
  return { tab, deps, claimed, translator, api, page: page.page, changes: () => changes };
}

/** Confirm the main-owned proposal, as an explicit click in the consent dialog would. */
async function approve(translator: Translator, tab: any) {
  await translator.translate(tab);
  const id = tab.translation?.confirmation?.id;
  if (id) await translator.confirm(tab, id);
}

/** A committed document, then its language detection and finished-load event. */
async function go(t: ReturnType<typeof setup>, url: string, lang = 'ja') {
  t.tab.url = url;
  t.page.lang = lang;
  t.page.document += '-next';
  t.translator.navigated(t.tab, url);
  await t.translator.detect(t.tab);
  t.translator.loaded(t.tab);
  await new Promise(setImmediate);
}

describe('Translator', () => {
  it('offers a page in a language the person does not read, named for the OS language', async () => {
    const { tab, translator } = setup();
    await translator.detect(tab);
    assert.deepEqual(tab.translation, { state: 'offer', lang: 'ja', target: 'en-US' });
  });

  it('offers nothing for a page in any of the OS languages, or one that declares none', async () => {
    for (const lang of ['en-GB', 'fr-CA', '']) {
      const { tab, translator } = setup(contents(lang, ['x']));
      await translator.detect(tab);
      assert.equal(tab.translation, undefined, lang);
    }
  });

  it('never offers or translates when governed, under a provider or while recording', async () => {
    const cases = [
      { governance: { configuration: { policies: [] } } },
      { config: { values: { provider: 'oya-cloud' } } },
      { recorder: { recording: true } },
    ];
    for (const over of cases) {
      const { tab, translator, api } = setup(undefined, undefined, over);
      await translator.detect(tab);
      await approve(translator, tab);
      assert.equal(tab.translation, undefined);
      assert.equal(api.sent.length, 0);
    }
  });

  it('never offers on the start page or a non-web address', async () => {
    const { tab, translator } = setup();
    tab.url = 'file:///etc/hosts';
    await translator.detect(tab);
    assert.equal(tab.translation, undefined);
  });

  it('translates the page, writes the answers back, and says it is done', async () => {
    const { tab, translator, api, page } = setup();
    tab.pageLang = 'ja';
    await approve(translator, tab);
    assert.deepEqual(api.sent, [{ texts: ['こんにちは'], target: 'en-US' }]);
    assert.deepEqual(page.applied, [['こんにちは'.toUpperCase()]]);
    assert.equal(tab.translation.state, 'translated');
  });

  it('says a page longer than the limit was translated in part', async () => {
    const { tab, translator } = setup(contents('ja', ['a'], true));
    await approve(translator, tab);
    assert.equal(tab.translation.state, 'partial');
  });

  it('refuses a tab an agent holds: nothing is sent', async () => {
    const { tab, translator, api, claimed } = setup();
    claimed.add(1);
    await approve(translator, tab);
    assert.equal(api.sent.length, 0);
  });

  it('says signed_out when this browser cannot call the server', async () => {
    const api = { ...server(), canCall: () => false };
    const { tab, translator } = setup(undefined, api);
    await approve(translator, tab);
    assert.equal(tab.translation.error.code, 'signed_out');
  });

  it('carries the server’s code and message, ends keep-translating, and restores what was written', async () => {
    const refusal = async () => Response.json({ error: 'No model', code: 'llm_unconfigured' }, { status: 422 });
    const { tab, translator, page } = setup(contents('ja', ['a', 'b'.repeat(TRANSLATE_BATCH_CHARS)]), server(refusal));
    tab.translateKeep = { lang: 'ja', origin: 'https://example.jp', url: '', pages: 0 };
    await approve(translator, tab);
    assert.deepEqual(tab.translation.error, { code: 'llm_unconfigured', message: 'No model' });
    assert.equal(tab.translateKeep, undefined);
    assert.equal(page.restored, 1);
  });

  it('says offline when the server cannot be reached', async () => {
    const down = async (): Promise<Response> => {
      throw new TypeError('fetch failed');
    };
    const { tab, translator } = setup(undefined, server(down));
    await approve(translator, tab);
    assert.equal(tab.translation.error.code, 'offline');
  });

  it('says nothing_to_translate for a page with no text', async () => {
    const { tab, translator } = setup(contents('ja', []));
    await approve(translator, tab);
    assert.equal(tab.translation.error.code, 'nothing_to_translate');
  });

  it('never writes a batch that arrives after the tab navigated away', async () => {
    let answer!: (r: Response) => void;
    const late = () => new Promise<Response>((resolve) => (answer = resolve));
    const { tab, translator, page } = setup(undefined, server(late));
    const running = approve(translator, tab);
    await new Promise(setImmediate);
    translator.navigated(tab);
    answer(Response.json({ texts: ['HELLO'] }));
    await running;
    assert.deepEqual(page.applied, []);
    assert.equal(tab.translation, undefined);
  });

  /** Translates the tab once (the click), then lands it on `url` declaring `lang` and lets the load finish. */
  async function clickThenGo(t: ReturnType<typeof setup>, url: string, lang = 'ja') {
    t.tab.pageLang = 'ja';
    await approve(t.translator, t.tab);
    await go(t, url, lang);
  }

  it('keeps translating the next page of the same site in the same language', async () => {
    const t = setup();
    await clickThenGo(t, 'https://example.jp/next');
    assert.equal(t.api.sent.length, 2);
  });

  it('does not keep translating a page in another language, on another site, or a reload of the same page', async () => {
    for (const [url, lang] of [
      ['https://example.jp/next', 'de'],
      ['https://bank.jp/', 'ja'],
      ['https://example.jp/', 'ja'],
    ]) {
      const t = setup();
      await clickThenGo(t, url, lang);
      assert.equal(t.api.sent.length, 1, `${url} ${lang}`);
    }
  });

  it('stops keeping on its own after the page cap, until the next click', async () => {
    const t = setup();
    await clickThenGo(t, 'https://example.jp/1');
    t.tab.translateKeep.pages = TRANSLATE_KEEP_PAGES;
    await go(t, 'https://example.jp/2');
    assert.equal(t.api.sent.length, 2);
    assert.equal(t.tab.translateKeep, undefined);
  });

  it('keeps translating a page that finished loading before its language was read', async () => {
    const source = contents('ja', ['a']);
    const t = setup(source);
    await approve(t.translator, t.tab);
    t.tab.url = 'https://example.jp/next';
    t.translator.navigated(t.tab);
    t.translator.loaded(t.tab);
    assert.equal(t.api.sent.length, 1);
    source.webContents.isLoading = () => false;
    await t.translator.detect(t.tab);
    await new Promise(setImmediate);
    assert.equal(t.api.sent.length, 2);
  });

  it('returning after another origin or language requires fresh consent', async () => {
    for (const [url, lang] of [
      ['https://other.example/', 'ja'],
      ['https://example.jp/plain', ''],
    ]) {
      const t = setup();
      await approve(t.translator, t.tab);
      await go(t, url, lang);
      await go(t, 'https://example.jp/', 'ja');
      assert.equal(t.api.sent.length, 1);
      assert.equal(t.tab.translateKeep, undefined);
    }
  });

  it('is not offered under a persona that does not read the person’s language', async () => {
    const t = setup(undefined, undefined, { persona: { active: { navigator: { languages: ['de-DE', 'de'] } } } });
    await t.translator.detect(t.tab);
    await approve(t.translator, t.tab);
    assert.equal(t.tab.translation, undefined);
    assert.equal(t.api.sent.length, 0);
  });

  it('is offered under a persona that reads the person’s language', async () => {
    const t = setup(undefined, undefined, { persona: { active: { navigator: { languages: ['en-GB'] } } } });
    await t.translator.detect(t.tab);
    assert.equal(t.tab.translation.state, 'offer');
  });

  it('Show original restores the page, ends keep-translating, and offers again', async () => {
    const { tab, translator, page } = setup();
    tab.pageLang = 'ja';
    await approve(translator, tab);
    await translator.showOriginal(tab);
    assert.equal(page.restored, 1);
    assert.equal(tab.translateKeep, undefined);
    assert.equal(tab.translation.state, 'offer');
  });

  it('an agent’s claim restores the page, and a page command waits until it has', async () => {
    const { tab, translator, page, claimed } = setup();
    tab.pageLang = 'ja';
    await approve(translator, tab);
    let finish!: () => void;
    page.restore = new Promise((resolve) => (finish = resolve));
    claimed.add(1);
    let released = false;
    void translator.sync();
    const waiting = translator.released().then(() => (released = true));
    await new Promise(setImmediate);
    assert.equal(released, false);
    finish();
    await waiting;
    assert.equal(page.restored, 1);
    assert.equal(tab.translation, undefined);
    assert.equal(tab.translateKeep, undefined);
  });

  it('offers again once the agent or the recording hands the tab back', async () => {
    const { tab, translator, claimed } = setup();
    await translator.detect(tab);
    claimed.add(1);
    await translator.sync();
    assert.equal(tab.translation, undefined);
    claimed.delete(1);
    await translator.sync();
    assert.equal(tab.translation.state, 'offer');
  });
});

describe('batches', () => {
  it('makes the first batch small, then fills each within the character limit, in order, with its first index', () => {
    const half = 'x'.repeat(TRANSLATE_BATCH_CHARS / 2);
    const small = 'y'.repeat(TRANSLATE_FIRST_BATCH_CHARS / 2);
    assert.deepEqual(
      batches([small, small, small, half, half, 'z']).map(([start, texts]) => [start, texts.length]),
      [
        [0, 2],
        [2, 2],
        [4, 2],
      ],
    );
  });
});

describe('translatePage in parallel', () => {
  it('has up to TRANSLATE_PARALLEL requests in flight at once', async () => {
    const texts = Array.from({ length: 6 }, (_, i) => String(i).repeat(TRANSLATE_BATCH_CHARS));
    let open = 0;
    let most = 0;
    const slow = async (body: any) => {
      most = Math.max(most, ++open);
      await new Promise(setImmediate);
      open--;
      return Response.json({ texts: body.texts });
    };
    const t = setup(contents('ja', texts), server(slow));
    await approve(t.translator, t.tab);
    assert.equal(most, TRANSLATE_PARALLEL);
    assert.equal(t.tab.translation.state, 'translated');
  });

  it('stops the other requests after one fails, so nothing is written after the restore', async () => {
    const texts = Array.from({ length: 4 }, (_, i) => String(i).repeat(TRANSLATE_BATCH_CHARS));
    let release!: () => void;
    const held = new Promise<void>((resolve) => (release = resolve));
    const answer = async (body: any) => {
      if (body.texts[0].startsWith('0'))
        return Response.json({ error: 'No', code: 'llm_unavailable' }, { status: 502 });
      await held;
      return Response.json({ texts: body.texts });
    };
    const page = contents('ja', texts);
    const t = setup(page, server(answer));
    const running = approve(t.translator, t.tab);
    await new Promise(setImmediate);
    release();
    await running;
    await new Promise(setImmediate);
    assert.equal(t.tab.translation.error.code, 'llm_unavailable');
    assert.deepEqual(page.page.applied, []);
  });
});

describe('failureOf', () => {
  it('names a timeout, a coded refusal, an uncoded 429 and anything else', () => {
    assert.equal(failureOf(Object.assign(new Error('t'), { name: 'TimeoutError' })).code, 'timeout');
    assert.equal(failureOf({ code: 'llm_quota', message: 'm' }).code, 'llm_quota');
    assert.equal(failureOf({ status: 429, message: 'm' }).code, 'rate_limited');
    assert.equal(failureOf({ status: 500, message: 'm' }).code, 'failed');
  });
});

describe('translation consent and document ownership', () => {
  it('sends no text before the exact proposal is explicitly confirmed', async () => {
    const t = setup();
    await t.translator.translate(t.tab);
    assert.equal(t.tab.translation.state, 'confirming');
    assert.equal(t.api.sent.length, 0);
    const id = t.tab.translation.confirmation.id;
    await t.translator.confirm(t.tab, 'unrelated-confirmation');
    assert.equal(t.api.sent.length, 0);
    await t.translator.confirm(t.tab, id);
    await t.translator.confirm(t.tab, id);
    assert.equal(t.api.sent.length, 1);
  });

  it('cancelling confirmation sends nothing and revokes the proposal', async () => {
    const t = setup();
    await t.translator.translate(t.tab);
    const id = t.tab.translation.confirmation.id;
    await t.translator.showOriginal(t.tab);
    await t.translator.confirm(t.tab, id);
    assert.equal(t.api.sent.length, 0);
  });

  it('a same-address reload requires a new confirmation and an old reply cannot dismiss it', async () => {
    const t = setup();
    await t.translator.translate(t.tab);
    const old = t.tab.translation.confirmation.id;
    t.translator.navigated(t.tab);
    await t.translator.translate(t.tab);
    const current = t.tab.translation.confirmation.id;
    await t.translator.confirm(t.tab, old);
    assert.equal(t.tab.translation.confirmation.id, current);
    assert.equal(t.api.sent.length, 0);
    await t.translator.confirm(t.tab, current);
    assert.equal(t.api.sent.length, 1);
  });

  it('takeover, recording, source or target changes revoke an unaccepted confirmation', async () => {
    const changes = [
      (t: ReturnType<typeof setup>) => t.claimed.add(t.tab.id),
      (t: ReturnType<typeof setup>) => {
        t.deps.recorder.recording = true;
      },
      (t: ReturnType<typeof setup>) => {
        t.page.lang = 'de';
      },
      (t: ReturnType<typeof setup>) => {
        t.deps.electron.app.getPreferredSystemLanguages = () => ['fr'];
      },
    ];
    for (const change of changes) {
      const t = setup();
      await t.translator.translate(t.tab);
      const id = t.tab.translation.confirmation.id;
      change(t);
      await t.translator.confirm(t.tab, id);
      assert.equal(t.api.sent.length, 0);
    }
  });

  it('unknown source language discloses and authorizes only this document', async () => {
    const t = setup(contents('', ['hello']));
    await t.translator.translate(t.tab);
    assert.equal(t.tab.translation.confirmation.pages, 0);
    await t.translator.confirm(t.tab, t.tab.translation.confirmation.id);
    assert.equal(t.tab.translateKeep, undefined);
  });

  it('a late detection cannot overwrite the replacement document language', async () => {
    const t = setup();
    let release!: () => void;
    t.page.identify = new Promise((resolve) => {
      release = resolve;
    });
    const old = t.translator.detect(t.tab);
    t.translator.navigated(t.tab);
    t.page.lang = 'de';
    t.page.identify = Promise.resolve();
    await t.translator.detect(t.tab);
    release();
    await old;
    assert.equal(t.tab.pageLang, 'de');
    assert.equal(t.tab.translation.lang, 'de');
  });

  it('a failed live restoration blocks reads until a retry succeeds or the document is replaced', async () => {
    const t = setup();
    await approve(t.translator, t.tab);
    t.page.restore = Promise.reject(Error('renderer refused'));
    await assert.rejects(t.translator.released(t.tab.view), { code: 'restore_failed' });
    assert.equal(t.translator.translated(t.tab), true);
    await assert.rejects(t.translator.released(t.tab.view), { code: 'restore_failed' });
    t.translator.navigated(t.tab);
    await t.translator.released(t.tab.view);
  });

  it('explicit restore failure offers reload while retaining the agent barrier', async () => {
    const t = setup();
    await approve(t.translator, t.tab);
    t.page.restore = Promise.reject(Error('renderer refused'));
    await assert.rejects(t.translator.showOriginal(t.tab), { code: 'restore_failed' });
    assert.equal(t.tab.translation.error.code, 'restore_failed');
    assert.equal(t.translator.translated(t.tab), true);
  });

  it('recording restores translated tabs in other windows', async () => {
    const t = setup();
    await approve(t.translator, t.tab);
    t.deps.windows = { allTabs: () => [t.tab] };
    t.deps.tabs.list = [];
    t.deps.recorder.recording = true;
    await t.translator.sync();
    assert.equal(t.page.restored, 1);
    assert.equal(t.tab.translation, undefined);
    assert.equal(t.tab.translateKeep, undefined);
  });

  it('malformed model replies never reach page writes', async () => {
    const t = setup(
      undefined,
      server(async () => Response.json({ texts: [{}] })),
    );
    await approve(t.translator, t.tab);
    assert.equal(t.tab.translation.error.code, 'translate_malformed');
    assert.deepEqual(t.page.applied, []);
  });
});
