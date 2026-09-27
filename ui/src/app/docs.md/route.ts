/**
 * /docs.md: the whole docs page as Markdown, for agents.
 */
import { fullDocsResponse } from '../docs/_docs/markdown-route';

/** The docs as one Markdown file. */
export function GET() {
  return fullDocsResponse();
}
