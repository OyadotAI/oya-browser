/**
 * Unit tests for the docs layout's metadata: it sits above the topic pages, so its title has to
 * carry a template or every topic page loses the site name from its title.
 */
import { describe, it, expect } from 'vitest';
import { metadata } from '@/app/docs/layout';

describe('the docs layout metadata', () => {
  it('keeps the title /docs has always had', () => {
    expect(metadata.title).toMatchObject({ default: 'Docs, API, personas, MCP and self-hosting' });
  });

  it('gives the pages under it a title template ending in the site name', () => {
    expect(metadata.title).toMatchObject({ template: '%s · Oya Browser' });
  });
});
