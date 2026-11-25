/** /vs/browser-use: Oya next to Browser Use, in facts about Oya only, for people choosing a browser for AI agents. */
import { AnswerPage, answerMetadata } from '../../_answers/answer-page';
import { VS_BROWSER_USE } from '../../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(VS_BROWSER_USE);

/** The Oya vs Browser Use page. */
export default function VsBrowserUse() {
  return <AnswerPage content={VS_BROWSER_USE} />;
}
