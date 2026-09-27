/**
 * /llms-full.txt: every docs page in one plain-text file, the llms.txt
 * convention's companion to the /llms.txt index.
 */
import { fullDocsResponse } from '../docs/_docs/markdown-route';

/** The docs, concatenated. */
export function GET() {
  return fullDocsResponse('text/plain; charset=utf-8');
}
