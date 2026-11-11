/** /docs/anonymity: the stealth and anonymity part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { AnonymityDocs } from '../_sections/anonymity';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('anonymity');

/** The stealth and anonymity docs page. */
export default function AnonymityTopic() {
  return (
    <TopicPage slug="anonymity">
      <AnonymityDocs />
    </TopicPage>
  );
}
