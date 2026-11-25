/** /vs/chrome-devtools-mcp: Oya next to Chrome DevTools MCP, from each project's own public pages. */
import { AnswerPage, answerMetadata } from '../../_answers/answer-page';
import { VS_CHROME_DEVTOOLS_MCP } from '../../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(VS_CHROME_DEVTOOLS_MCP);

/** The Oya vs Chrome DevTools MCP page. */
export default function VsChromeDevtoolsMcp() {
  return <AnswerPage content={VS_CHROME_DEVTOOLS_MCP} />;
}
