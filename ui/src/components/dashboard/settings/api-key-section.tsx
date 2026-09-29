/**
 * The API key panel: the open project's key, to paste into an SDK, MCP client
 * or script. Other projects' keys stay in the header's project menu.
 */
'use client';

import { ProjectKey } from '../project-switcher';
import { SectionHeading } from './heading';

/** The heading, the show-and-copy button and where to find other projects' keys. */
export default function ApiKeySection({ projectId }: { /** The open project. */ projectId: string | null }) {
  return (
    <>
      <SectionHeading eyebrow="API key" title="Connect your code.">
        This project&apos;s key signs in the SDK, the CLI and MCP clients. Keep it private: it can start browsers and
        read everything in this project.
      </SectionHeading>
      <ProjectKey projectId={projectId} />
      <p className="mt-4 text-xs text-text-muted">
        Other projects&apos; keys are in the project menu at the top of the page, under each project&apos;s ⋯ button.
      </p>
    </>
  );
}
