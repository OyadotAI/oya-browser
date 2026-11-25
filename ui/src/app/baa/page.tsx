/** /baa: how to get a Business Associate Agreement for Oya, and what HIPAA mode does. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { BAA } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(BAA);

/** How to get a Business Associate Agreement for Oya, and what HIPAA mode does. */
export default function BaaPage() {
  return <AnswerPage content={BAA} />;
}
