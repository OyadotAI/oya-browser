/** /vs/playwright-mcp: Oya next to Playwright MCP, from each project's own public pages. */
import { AnswerPage, answerMetadata } from '../../_answers/answer-page';
import { VS_PLAYWRIGHT_MCP } from '../../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(VS_PLAYWRIGHT_MCP);

/** The Oya vs Playwright MCP page. */
export default function VsPlaywrightMcp() {
  return <AnswerPage content={VS_PLAYWRIGHT_MCP} />;
}
