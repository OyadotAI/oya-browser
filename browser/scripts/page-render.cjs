/**
 * Facade for page rendering. The analyzer (scripts/analyzer.js) collects a page
 * as data: its facts, one row per block in reading order, and its elements. A
 * renderer writes that data as text, one renderer per format, all with the same
 * interface:
 *
 *   render(analysis)        the page as text
 *   fit(text, max)          that text cut to `max` characters, still valid
 *   forAgent(analysis, max) what a model reads, within its budget
 *
 * createRenderer is the factory that picks one from configuration. A new format
 * is a file in page-render/ and a line in RENDERERS. The desktop app and the
 * server both use this file.
 */
const markdown = require('./page-render/markdown.cjs');
const toon = require('./page-render/toon.cjs');
const jsonl = require('./page-render/jsonl.cjs');
const { PAGE_RENDER } = require('./constants.cjs');

/** Every renderer, by the format name configuration uses. */
const RENDERERS = { markdown, toon, jsonl };

/** The formats there are. */
const FORMATS = Object.keys(RENDERERS);

/** The format when none is configured: markdown, what analysis has always returned. */
const DEFAULT_FORMAT = 'markdown';

/** The facts in reading order, whatever order they arrived in (a trip through the isolated world sorts them). */
const FACT_ORDER = [
  'url',
  'title',
  'viewport',
  'scroll',
  'panelScroll',
  'elements',
  'modal',
  'covered',
  'focused',
  'truncated',
];

/** The facts in FACT_ORDER, any others after them. */
const ordered = (facts = {}) =>
  Object.fromEntries(
    [...FACT_ORDER.filter((key) => key in facts), ...Object.keys(facts).filter((key) => !FACT_ORDER.includes(key))].map(
      (key) => [key, facts[key]],
    ),
  );

/** The renderer for `format` (the default when none is given); an unknown one is refused, naming the known ones. */
function createRenderer(format) {
  const name = format || DEFAULT_FORMAT;
  if (!Object.hasOwn(RENDERERS, name))
    throw new Error(`Unknown page format "${name}". Use one of: ${FORMATS.join(', ')}`);
  return RENDERERS[name];
}

/**
 * An analyze result with its page written in `format`: `page` and `format`, and
 * `markdown` as well when the format is markdown, as analysis has always had it.
 * A result that is not an analysis (an error, an older analyzer) passes through.
 */
function withPage(result, format) {
  if (!result?.ok || !Array.isArray(result.data?.blocks)) return result;
  const renderer = createRenderer(format);
  const data = { ...result.data, facts: ordered(result.data.facts) };
  const page = renderer.fit(renderer.render(data), PAGE_RENDER.MAX_PAGE_CHARS);
  const alias = renderer.format === 'markdown' ? { markdown: page } : {};
  return { ...result, data: { ...data, format: renderer.format, page, ...alias } };
}

/** A saved analysis (its data, as withPage returned it) written again in `format`. */
const renderPage = (data, format) => withPage({ ok: true, data }, format).data.page;

module.exports = {
  createRenderer,
  renderPage,
  withPage,
  FORMATS,
  DEFAULT_FORMAT,
  elementIndex: markdown.elementIndex,
  toonCell: toon.toonCell,
  toonTable: toon.toonTable,
};
