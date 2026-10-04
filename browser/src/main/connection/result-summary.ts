/**
 * What the dev log shows for a command result: the answer's shape, not its
 * payload (a screenshot is its size, a page read is its first lines).
 */
import type { CommandId } from '../actions/types.ts';
import { ID_PREVIEW_CHARS, MARKDOWN_PREVIEW_CHARS, BYTES_PER_KB } from './constants.ts';

/** A result field's value: text, or a list whose summarizer reads only its length. */
type Field = string;

/** The log line's fields: the id, whether it worked, the error, and each summarized field. */
export type ResultSummary = Record<string, unknown>;

/** Result field → how it is summarized, in the order they are shown. */
const SUMMARIZERS: Record<string, (v: Field) => unknown> = {
  screenshot: (v) => `${Math.round(v.length / BYTES_PER_KB)}KB`,
  url: (v) => v,
  title: (v) => v,
  format: (v) => v,
  page: (v) => v.slice(0, MARKDOWN_PREVIEW_CHARS) + (v.length > MARKDOWN_PREVIEW_CHARS ? '...' : ''),
  blocks: (v) => `${v.length} blocks`,
  elements: (v) => `${v.length} elements`,
  tabs: (v) => `${v.length} tabs`,
  tab_id: (v) => v,
  viewport: (v) => v,
  scroll: (v) => v,
  dialog: (v) => v,
};

/** Each field the result carries, summarized, in SUMMARIZERS' order. */
function summarizeFields(fields: Record<string, Field>): ResultSummary {
  const summary: ResultSummary = {};
  for (const [key, summarize] of Object.entries(SUMMARIZERS)) if (fields[key]) summary[key] = summarize(fields[key]);
  return summary;
}

/** The summary of one command result. */
export function resultSummary(
  id: CommandId | undefined,
  ok: boolean,
  data: unknown,
  error?: string | null,
): ResultSummary {
  const summary: ResultSummary = { id: String(id).slice(0, ID_PREVIEW_CHARS), ok };
  if (error) summary.error = error;
  return data ? { ...summary, ...summarizeFields(data as Record<string, Field>) } : summary;
}
