/** /docs/playbooks: the playbooks part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { PlaybooksDocs } from '../_sections/playbooks';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('playbooks');

/** The playbooks docs page. */
export default function PlaybooksTopic() {
  return (
    <TopicPage slug="playbooks">
      <PlaybooksDocs />
    </TopicPage>
  );
}
