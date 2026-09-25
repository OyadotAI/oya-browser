/**
 * Unit tests for modules/config/openrouter-models.ts: OpenRouter's live list is
 * trimmed to models the agent can drive, served from memory for an hour, and
 * stood in for by the built-in list whenever it is not there. fetch is faked.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  openRouterModels,
  resetOpenRouterModels,
  settled,
  toChoices,
} from '../../../../src/modules/config/openrouter-models.ts';
import { OPENROUTER_MODELS_TTL_MS } from '../../../../src/modules/config/constants.ts';

const URL = 'https://openrouter.test/api/v1/models';
const FALLBACK = [{ id: 'built/in', label: 'Built-in' }];

/** One model as OpenRouter lists it. */
const model = (id: string, created: number, tools = true) => ({
  id,
  name: `Name ${id}`,
  created,
  supported_parameters: tools ? ['tools', 'temperature'] : ['temperature'],
});

/** A fetch that answers with `data`, counting its calls. */
const fakeFetch = (data: unknown[], status = 200) =>
  mock.fn(async () => new Response(JSON.stringify({ data }), { status })) as unknown as typeof fetch & {
    mock: { callCount(): number };
  };

describe('toChoices', () => {
  it('keeps models that take tool calls, drops batch variants, newest first', () => {
    const choices = toChoices([
      model('a/old', 1),
      model('a/new', 3),
      model('a/no-tools', 4, false),
      model('a/new:batch', 5),
    ]);
    assert.deepEqual(choices, [
      { id: 'a/new', label: 'Name a/new' },
      { id: 'a/old', label: 'Name a/old' },
    ]);
  });
});

describe('toChoices: order', () => {
  it('leads with the hand-picked models OpenRouter still serves, in their order, then the rest newest first', () => {
    const pinned = [
      { id: 'a/pick-2', label: 'x' },
      { id: 'a/gone', label: 'x' },
      { id: 'a/pick-1', label: 'x' },
    ];
    const data = [model('a/pick-1', 1), model('a/new', 9), model('a/pick-2', 2), model('a/older', 5)];
    assert.deepEqual(
      toChoices(data, pinned).map((c) => c.id),
      ['a/pick-2', 'a/pick-1', 'a/new', 'a/older'],
    );
  });
});

describe('openRouterModels', () => {
  beforeEach(() => {
    resetOpenRouterModels();
    mock.timers.enable({ apis: ['Date'] });
  });
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
  });

  it('answers with the built-in list at once, then with the live one once it lands', async () => {
    const fetch = fakeFetch([model('live/one', 1)]);
    assert.deepEqual(openRouterModels(FALLBACK, fetch, URL), FALLBACK, 'never waits on OpenRouter');
    await settled();
    assert.deepEqual(openRouterModels(FALLBACK, fetch, URL), [{ id: 'live/one', label: 'Name live/one' }]);
  });

  it('fetches once an hour, not on every read', async () => {
    const fetch = fakeFetch([model('live/one', 1)]);
    openRouterModels(FALLBACK, fetch, URL);
    await settled();
    openRouterModels(FALLBACK, fetch, URL);
    assert.equal(fetch.mock.callCount(), 1);
    mock.timers.tick(OPENROUTER_MODELS_TTL_MS);
    openRouterModels(FALLBACK, fetch, URL);
    await settled();
    assert.equal(fetch.mock.callCount(), 2);
  });

  it('keeps the built-in list when OpenRouter fails or is unreachable', async () => {
    openRouterModels(FALLBACK, fakeFetch([], 503), URL);
    await settled();
    assert.deepEqual(openRouterModels(FALLBACK, fakeFetch([]), ''), FALLBACK);
    const refused = mock.fn(async () => {
      throw new Error('offline');
    }) as unknown as typeof fetch;
    mock.timers.tick(OPENROUTER_MODELS_TTL_MS);
    openRouterModels(FALLBACK, refused, URL);
    await settled();
    assert.deepEqual(openRouterModels(FALLBACK, refused, ''), FALLBACK);
  });

  it('never fetches when the URL is turned off', () => {
    const fetch = fakeFetch([model('live/one', 1)]);
    assert.deepEqual(openRouterModels(FALLBACK, fetch, ''), FALLBACK);
    assert.equal(fetch.mock.callCount(), 0);
  });
});
