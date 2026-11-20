/** /browser-automation-for-regulated-industries: the controls a reviewer checks before an agent works regulated data. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { REGULATED_INDUSTRIES } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(REGULATED_INDUSTRIES);

/** The controls Oya offers regulated teams, and the one regulation it has a contract for. */
export default function RegulatedIndustriesPage() {
  return <AnswerPage content={REGULATED_INDUSTRIES} />;
}
