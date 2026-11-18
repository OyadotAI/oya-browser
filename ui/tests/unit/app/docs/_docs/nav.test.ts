/** Documentation anchors honor the reader's motion preference and tolerate unavailable sections. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { scrollToSection } from '@/app/docs/_docs/nav';

afterEach(() => {
  document.body.innerHTML = '';
  vi.unstubAllGlobals();
});

describe('documentation anchor scrolling', () => {
  it.each([
    [true, 'auto'],
    [false, 'smooth'],
  ])('uses the motion preference %s', (matches, behavior) => {
    const section = document.createElement('section');
    section.id = 'healing';
    section.scrollIntoView = vi.fn();
    document.body.append(section);
    vi.stubGlobal('matchMedia', () => ({ matches }));
    scrollToSection('healing');
    expect(section.scrollIntoView).toHaveBeenCalledWith({ behavior, block: 'start' });
  });
  it('ignores an unavailable target and tolerates an unavailable media query API', () => {
    vi.stubGlobal('matchMedia', undefined);
    expect(() => scrollToSection('missing')).not.toThrow();
  });
});
