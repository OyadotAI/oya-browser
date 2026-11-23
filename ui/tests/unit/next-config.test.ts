/**
 * The short healthcare and comparison addresses redirect permanently to the pages that already
 * rank, so a guessed URL lands and no duplicate page competes for the same search.
 */
import { describe, expect, it } from 'vitest';
import config from '../../next.config';

/** The redirects the config declares. */
const redirects = () => config.redirects!();

describe('next.config redirects', () => {
  it('sends each healthcare alias to its existing page, permanently', async () => {
    const list = await redirects();
    const to = (source: string) => list.find((r) => r.source === source);
    expect(to('/security')).toMatchObject({ destination: '/trust', permanent: true });
    expect(to('/hipaa')).toMatchObject({ destination: '/baa', permanent: true });
    expect(to('/healthcare')).toMatchObject({ destination: '/browser-for-healthcare', permanent: true });
    expect(to('/use-cases/prior-authorization')).toMatchObject({ destination: '/prior-authorization-automation' });
  });

  it('maps every /compare/<vendor> onto /vs/<vendor>', async () => {
    expect(await redirects()).toContainEqual({
      source: '/compare/:vendor',
      destination: '/vs/:vendor',
      permanent: true,
    });
  });
});
