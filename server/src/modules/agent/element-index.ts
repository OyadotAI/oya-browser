/**
 * The element index appended to an analysis, written as TOON tables
 * (see toon.ts): every form field with its label, value, expected format and
 * state; the other visible elements; and a capped list of off-screen ones. The
 * agent fills forms from the field table, so it carries what the page shows
 * about each field: required, invalid and the page's error for it.
 */
import pageRender from '../../../../browser/scripts/page-render.cjs';
import { MAX_ANALYSIS_CHARS, READ_ELEMENTS_LIMIT, PAGE_FORMAT } from './constants.ts';

/** The element index (TOON tables of fields and elements), shared with the markdown page renderer. */
export const elementIndex: (elements: any[], truncated?: boolean) => string = pageRender.elementIndex;

/** Said where an older analyzer's markdown was cut to leave room for the index. */
const CUT = '\n\n⚠ Output truncated to fit context window.';

/** An older desktop app's analysis (markdown, no blocks): its markdown cut on a line, then the index. */
function legacyText({ markdown = '', elements = [], truncated }) {
  const index = elementIndex(elements, truncated);
  const room = MAX_ANALYSIS_CHARS - index.length;
  if (markdown.length <= room) return markdown + index;
  const cut = markdown.slice(0, Math.max(room - CUT.length, 0));
  return cut.slice(0, cut.lastIndexOf('\n') > 0 ? cut.lastIndexOf('\n') : cut.length) + CUT + index;
}

/**
 * The analysis as the model reads it, within MAX_ANALYSIS_CHARS, in the format the
 * browser wrote it in (else markdown). The renderer decides what gives way; an
 * older analyzer without blocks gets its markdown and the element index. Asked
 * for no content, it answers with the elements alone, which is much shorter.
 */
export function analysisText(data, { content = true } = {}) {
  if (!Array.isArray(data?.blocks)) return legacyText(data || {});
  const renderer = pageRender.createRenderer(data.format);
  if (!content) return renderer.controls(data, MAX_ANALYSIS_CHARS);
  return renderer.forAgent(data, MAX_ANALYSIS_CHARS);
}

/** Leads the guides when the format is not pinned, so the agent reads whichever it gets. */
const UNPINNED_GUIDE =
  '- The browser picks the page format (markdown, TOON or JSONL); read whichever analyze_page returns:';

/** How the agent is told to read the page: the pinned format's guide, or every format's when none is pinned. */
export function pageGuide(): string {
  if (PAGE_FORMAT) return pageRender.createRenderer(PAGE_FORMAT).guide;
  const guides = pageRender.FORMATS.map((format: string) => pageRender.createRenderer(format).guide);
  return [UNPINNED_GUIDE, ...guides].join('\n');
}

/**
 * read_elements' answer: the page's name and the index of its first `limit`
 * elements, from an analysis, so the ids are ones click and type accept.
 */
export function elementList({ url, title, elements }, limit = READ_ELEMENTS_LIMIT) {
  return `Page: ${title} (${url})` + elementIndex(elements.slice(0, limit), false);
}
