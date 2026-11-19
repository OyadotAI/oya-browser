/** Documentation anchors honor the reader's motion preference and tolerate unavailable sections. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';
import { scrollToSection, useDocsNavState } from '@/app/docs/_docs/nav';
import { ACTIVE_LINK_MARGIN_PX } from '@/app/docs/_docs/constants';

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

/** A side menu 600px tall holding the current link at the given top edge. */
function menuWithLinkAt(top: number) {
  const menu = document.createElement('aside');
  const link = document.createElement('button');
  link.setAttribute('aria-current', 'location');
  menu.append(link);
  document.body.append(menu);
  menu.getBoundingClientRect = () => ({ top: 0, bottom: 600 }) as DOMRect;
  link.getBoundingClientRect = () => ({ top, bottom: top + 30 }) as DOMRect;
  return menu;
}

describe('the side menu follows the page', () => {
  it('scrolls the menu down when the current link is below its edge', () => {
    const menu = menuWithLinkAt(700);
    renderHook(() => useDocsNavState(() => {}));
    expect(menu.scrollTop).toBe(730 - (600 - ACTIVE_LINK_MARGIN_PX));
  });
  it('scrolls the menu up when the current link is above its edge', () => {
    const menu = menuWithLinkAt(-50);
    menu.scrollTop = 400;
    renderHook(() => useDocsNavState(() => {}));
    expect(menu.scrollTop).toBe(400 - (ACTIVE_LINK_MARGIN_PX + 50));
  });
  it('leaves the menu alone when the current link is already in view', () => {
    const menu = menuWithLinkAt(300);
    menu.scrollTop = 120;
    renderHook(() => useDocsNavState(() => {}));
    expect(menu.scrollTop).toBe(120);
  });
});
