/** /vs/skyvern: Oya next to Skyvern, from each vendor's own public pages. */
import { AnswerPage, answerMetadata } from '../../_answers/answer-page';
import { VS_SKYVERN } from '../../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(VS_SKYVERN);

/** The Oya vs Skyvern page. */
export default function VsSkyvern() {
  return <AnswerPage content={VS_SKYVERN} />;
}
