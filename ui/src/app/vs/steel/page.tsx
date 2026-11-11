/** /vs/steel: Oya next to Steel, in facts about Oya only, for people choosing a browser for AI agents. */
import { AnswerPage, answerMetadata } from '../../_answers/answer-page';
import { VS_STEEL } from '../../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(VS_STEEL);

/** The Oya vs Steel page. */
export default function VsSteel() {
  return <AnswerPage content={VS_STEEL} />;
}
