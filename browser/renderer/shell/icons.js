/**
 * Desktop shell. IPC is provided only to this renderer, never to visited pages.
 *
 * The shell's line icons, drawn inline, and filling every [data-icon] and
 * action button on load.
 */
/* global Dom */
/* exported ShellIcons */

/** The icon set. */
const ShellIcons = {
  /** SVG path data by icon name. */
  PATHS: {
    back: 'm14 5-7 7 7 7M7 12h13',
    forward: 'm10 5 7 7-7 7M4 12h13',
    reload: 'M20 7v5h-5M19 12a7 7 0 1 1-2-5l3 3',
    plus: 'M12 5v14M5 12h14',
    close: 'm6 6 12 12M6 18 18 6',
    menu: 'M5 6h14M5 12h14M5 18h14',
    globe: 'M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0ZM3 12h18M12 3c-5 5-5 13 0 18 5-5 5-13 0-18Z',
    record: 'M19 12a7 7 0 1 1-14 0 7 7 0 0 1 14 0Z',
    stop: 'M6 6h12v12H6Z',
    panel: 'M3 4h18v16H3ZM15 4v16',
    send: 'm5 12 7-7 7 7M12 5v15',
    spark: 'm12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5Z',
    chevron: 'm9 5 7 7-7 7',
    profile: 'M16 8a4 4 0 1 1-8 0 4 4 0 0 1 8 0ZM4 21v-2a8 8 0 0 1 16 0v2',
    check: 'm5 12 4 4L19 6',
    copy: 'M9 9h11v11H9ZM5 15H4V4h11v1',
    playbook: 'M6 3h12v18l-6-4-6 4Z',
    code: 'm8 7-5 5 5 5m8-10 5 5-5 5',
    scan: 'M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5M7 8h10M7 12h10M7 16h6',
    camera: 'M3 7h4l2-3h6l2 3h4v13H3ZM16 13a4 4 0 1 1-8 0 4 4 0 0 1 8 0Z',
    down: 'M12 4v16m-6-6 6 6 6-6',
    up: 'M12 20V4m-6 6 6-6 6 6',
  },

  /** The icon each Actions button shows. */
  ACTION_ICONS: {
    analyze: 'scan',
    screenshot: 'camera',
    reload: 'reload',
    'scroll-down': 'down',
    'scroll-up': 'up',
    'list-tabs': 'panel',
  },

  /** An icon's SVG markup; an unknown name draws the code icon. */
  icon(name) {
    const path = Object.hasOwn(ShellIcons.PATHS, name) ? ShellIcons.PATHS[name] : ShellIcons.PATHS.code;
    return `<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${path}"/></svg>`;
  },

  /** The icon for an Actions button's glyph. */
  actionIcon(el) {
    const action = el.closest('[data-action]')?.dataset.action;
    return ShellIcons.icon(Object.hasOwn(ShellIcons.ACTION_ICONS, action) ? ShellIcons.ACTION_ICONS[action] : 'code');
  },
};

document.querySelectorAll('[data-icon]').forEach((el) => (el.innerHTML = ShellIcons.icon(el.dataset.icon)));
document.querySelectorAll('.action-icon').forEach((el) => (el.innerHTML = ShellIcons.actionIcon(el)));
document.querySelectorAll('.action-field').forEach((el) => {
  if (!el.hasAttribute('aria-label')) el.setAttribute('aria-label', el.placeholder || 'Action parameter');
});
Dom.byId('source-refresh').textContent = 'Refresh';
