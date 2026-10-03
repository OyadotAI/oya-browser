/**
 * Unit tests for where a visitor came from: an ad campaign's tags, then an ad
 * click id, then the linking site, then direct; and a tagged visit replacing
 * the kept source while an untagged one leaves it.
 */
import { describe, it, expect, beforeEach } from 'vitest';
import { rememberSource, visitSource } from '@/lib/visitor-source';

/** The source this visit would give, from a URL and a referrer. */
const from = (url: string, referrer = '') => visitSource(new URL(url), referrer).source;

/** The kept source, decoded, or '' when none is kept. */
const kept = () =>
  decodeURIComponent(
    document.cookie
      .split(';')
      .map((c) => c.trim())
      .find((c) => c.startsWith('oya_src='))
      ?.slice('oya_src='.length) ?? '',
  );

/** Opens `url` from `referrer`, as far as rememberSource can tell. */
function visit(url: string, referrer = '') {
  window.history.replaceState(null, '', url);
  Object.defineProperty(document, 'referrer', { value: referrer, configurable: true });
  rememberSource();
}

describe('visitSource', () => {
  it("names a tagged link by its UTM source and campaign, lower case and stripped of what a name doesn't need", () => {
    expect(from('https://oyabrowser.com/?utm_source=Facebook&utm_campaign=Fall<Launch>')).toBe('facebook / falllaunch');
  });

  it('names an untagged ad click by its network', () => {
    expect(from('https://oyabrowser.com/?fbclid=abc')).toBe('facebook ad');
    expect(from('https://oyabrowser.com/?gclid=abc')).toBe('google ad');
  });

  it('names the linking site, else says direct; this site is not a source', () => {
    expect(from('https://oyabrowser.com/', 'https://www.reddit.com/r/x')).toBe('reddit.com');
    expect(from('https://oyabrowser.com/docs', 'https://oyabrowser.com/')).toBe('direct');
    expect(from('https://oyabrowser.com/')).toBe('direct');
  });
});

describe('rememberSource', () => {
  beforeEach(() => {
    document.cookie = 'oya_src=; Max-Age=0; Path=/';
  });

  it('keeps the first untagged source and does not let a later untagged visit replace it', () => {
    visit('/', 'https://news.ycombinator.com/item?id=1');
    visit('/docs', 'https://www.google.com/');
    expect(kept()).toBe('news.ycombinator.com');
  });

  it('lets an ad or tagged visit replace whatever was kept', () => {
    visit('/', 'https://www.google.com/');
    visit('/?utm_source=facebook&utm_campaign=fall');
    expect(kept()).toBe('facebook / fall');
  });
});
