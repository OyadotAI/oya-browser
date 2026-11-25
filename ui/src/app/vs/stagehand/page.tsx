/** /vs/stagehand: Oya next to Stagehand, from each project's own public pages. */
import { AnswerPage, answerMetadata } from '../../_answers/answer-page';
import { VS_STAGEHAND } from '../../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(VS_STAGEHAND);

/** The Oya vs Stagehand page. */
export default function VsStagehand() {
  return <AnswerPage content={VS_STAGEHAND} />;
}
