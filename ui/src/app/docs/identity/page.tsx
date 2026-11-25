/** /docs/identity: the identity part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { IdentityDocs } from '../_sections/identity';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('identity');

/** The identity docs page. */
export default function IdentityTopic() {
  return (
    <TopicPage slug="identity">
      <IdentityDocs />
    </TopicPage>
  );
}
