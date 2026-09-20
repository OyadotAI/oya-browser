/**
 * analyze's result as an MCP client reads it: the page in the format the browser
 * wrote it in (markdown, toon, or any renderer added in
 * browser/scripts/page-render.cjs), cut to fit like the agent's.
 */
import { analysisText } from '../modules/agent/chat.ts';

/** An analysis, returned as text. */
export type Page = {
  /** The page in the requested format. */
  page: string;
};

/** The analysis as text within the context budget. */
export function analysis(data: Record<string, unknown>): Page {
  return { page: analysisText(data) };
}
