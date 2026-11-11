/** /docs/mcp: the MCP setup part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { AiIntegrationDocs } from '../_sections/ai-integration';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('mcp');

/** The MCP setup docs page. */
export default function McpTopic() {
  return (
    <TopicPage slug="mcp">
      <AiIntegrationDocs />
    </TopicPage>
  );
}
