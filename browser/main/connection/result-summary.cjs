/**
 * What the dev log shows for a command result: the answer's shape, not its
 * payload (a screenshot is its size, a page read is its first lines).
 */
const { ID_PREVIEW_CHARS, MARKDOWN_PREVIEW_CHARS, BYTES_PER_KB } = require('./constants.cjs');

/** Result field → how it is summarized, in the order they are shown. */
const SUMMARIZERS = {
  screenshot: (v) => `${Math.round(v.length / BYTES_PER_KB)}KB`,
  url: (v) => v,
  title: (v) => v,
  markdown: (v) => v.slice(0, MARKDOWN_PREVIEW_CHARS) + (v.length > MARKDOWN_PREVIEW_CHARS ? '...' : ''),
  elements: (v) => `${v.length} elements`,
  tabs: (v) => `${v.length} tabs`,
  tab_id: (v) => v,
  viewport: (v) => v,
  scroll: (v) => v,
  dialog: (v) => v,
};

/** The summary of one command result. */
function resultSummary(id, ok, data, error) {
  const summary = { id: id.slice(0, ID_PREVIEW_CHARS), ok };
  if (error) summary.error = error;
  if (!data) return summary;
  for (const [key, summarize] of Object.entries(SUMMARIZERS)) if (data[key]) summary[key] = summarize(data[key]);
  return summary;
}

module.exports = { resultSummary };
