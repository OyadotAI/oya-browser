/** /docs/mcp-tools: the MCP tools part of the docs as its own page. */
import { TopicPage, topicMetadata } from '../_docs/topic-page';
import { McpToolsDocs } from '../_sections/mcp-tools';

/** Its own title, description and canonical address. */
export const metadata = topicMetadata('mcp-tools');

/** The MCP tools docs page. */
export default function McpToolsTopic() {
  return (
    <TopicPage slug="mcp-tools">
      <McpToolsDocs />
    </TopicPage>
  );
}
