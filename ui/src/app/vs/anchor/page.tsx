/** /vs/anchor: Oya next to Anchor, in facts about Oya only, for people choosing a browser for AI agents. */
import { AnswerPage, answerMetadata } from '../../_answers/answer-page';
import { VS_ANCHOR } from '../../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(VS_ANCHOR);

/** The Oya vs Anchor page. */
export default function VsAnchor() {
  return <AnswerPage content={VS_ANCHOR} />;
}
