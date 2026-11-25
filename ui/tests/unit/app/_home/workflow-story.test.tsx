/** New marketing copy stays qualified and links to the evidence behind portal-document context. */
import { afterEach, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { WorkflowStory } from '@/app/_home/workflow-story';
import { MAINTENANCE_BODY, PORTAL_CONTEXT, PORTAL_SOURCES } from '@/app/_home/workflow-story-content';
import { CLAIM_STATUS, DENIALS, PAYER_PORTALS } from '@/app/_answers/content';
afterEach(cleanup);

it('keeps the full repair qualification in the disclosure and links to the explanation', () => {
  render(<WorkflowStory />);
  expect(screen.getByText(MAINTENANCE_BODY)).toBeTruthy();
  expect(screen.getByRole('link', { name: /How playbook repair works/ }).getAttribute('href')).toBe(
    '/docs/playbooks#healing',
  );
});

it('keeps the portal-document use case off the home story', () => {
  render(<WorkflowStory />);
  expect(screen.queryByText(PORTAL_CONTEXT.body)).toBeNull();
});

it('uses the same qualified portal context and dated primary sources on relevant guides', () => {
  for (const content of [CLAIM_STATUS, DENIALS, PAYER_PORTALS]) {
    expect(content.sections.some((section) => section.body === PORTAL_CONTEXT.body)).toBe(true);
    expect(content.sources).toEqual(PORTAL_SOURCES);
  }
});
