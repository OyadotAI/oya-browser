/** /browser-for-healthcare: The page for people looking for a browser for healthcare, with the BAA questions answered. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { HEALTHCARE_BROWSER } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(HEALTHCARE_BROWSER);

/** The page for people looking for a browser for healthcare, with the BAA questions answered. */
export default function BrowserForHealthcare() {
  return <AnswerPage content={HEALTHCARE_BROWSER} />;
}
