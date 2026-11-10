/** /vs/browserbase: Oya next to Browserbase, in facts about Oya only, for people choosing a browser for AI agents. */
import { AnswerPage, answerMetadata } from '../../_answers/answer-page';
import { VS_BROWSERBASE } from '../../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(VS_BROWSERBASE);

/** The Oya vs Browserbase page. */
export default function VsBrowserbase() {
  return <AnswerPage content={VS_BROWSERBASE} />;
}
