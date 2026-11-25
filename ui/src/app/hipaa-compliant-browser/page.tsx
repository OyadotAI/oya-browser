/** /hipaa-compliant-browser: the page that answers "what is a HIPAA-compliant browser?", and says Oya is one. */
import { AnswerPage, answerMetadata } from '../_answers/answer-page';
import { HIPAA_BROWSER } from '../_answers/content';

/** Its title, its answer as the description, and its canonical address. */
export const metadata = answerMetadata(HIPAA_BROWSER);

/** The HIPAA-compliant browser page. */
export default function HipaaCompliantBrowser() {
  return <AnswerPage content={HIPAA_BROWSER} />;
}
