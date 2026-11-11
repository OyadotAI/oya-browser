/** /docs/cli: the CLI part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { CliDocs } from '../_sections/getting-started';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('cli');

/** The CLI docs page. */
export default function CliTopic() {
  return (
    <TopicPage slug="cli">
      <CliDocs />
    </TopicPage>
  );
}
