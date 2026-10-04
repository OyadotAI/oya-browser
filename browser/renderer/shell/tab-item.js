/**
 * One tab on the strip: its icon (the page's favicon, a spinner while it
 * loads, the Oya mark on the start page, a globe otherwise), its title (the
 * role=tab button) and its close button, and the pointer gestures it answers:
 * press to show and drag, middle-click to close, right-click for its menu,
 * rest for its hover card.
 */
/* global oyaBrowser, Dom, ShellIcons, TabDrag, TabCard, TabStrip */
/* exported TabItem */

/** Icon kind → its markup; anything else is a favicon's data: URL. */
const TAB_ICONS = {
  loading: () => '<span class="tab-spinner"></span>',
  home: () => '<svg class="tab-mark" viewBox="0 0 512 512"><use href="#oya-mark"/></svg>',
  globe: () => ShellIcons.icon('globe'),
};

/** Building and drawing one tab. */
const TabItem = {
  /** A new tab item, wired to its gestures. */
  create(tab) {
    const item = Dom.node('div', null, 'tab-item');
    item.dataset.id = tab.id;
    const icon = Dom.node('span', null, 'tab-icon');
    icon.setAttribute('aria-hidden', 'true');
    item.append(icon, TabItem.titleButton(tab), TabItem.closeButton(tab));
    TabItem.wire(item, tab.id);
    return item;
  },

  /** A tab's title: the tab button itself (Enter or Space shows it; a pointer press already has). */
  titleButton(tab) {
    const title = Dom.node('button', null, 'tab-title');
    title.setAttribute('role', 'tab');
    title.addEventListener('click', () => oyaBrowser.activateTab(tab.id));
    return title;
  },

  /** A tab's close button; out of the Tab order, since Delete closes the focused tab. */
  closeButton(tab) {
    const close = Dom.node('button', null, 'tab-close');
    close.innerHTML = ShellIcons.icon('close');
    close.tabIndex = -1;
    close.addEventListener('click', () => TabStrip.closeByMouse(tab.id));
    return close;
  },

  /** The pointer gestures a tab answers. */
  wire(item, id) {
    item.addEventListener('pointerdown', (event) => TabItem.press(event, item));
    item.addEventListener('pointermove', TabDrag.move);
    item.addEventListener('pointerup', TabDrag.release);
    item.addEventListener('pointercancel', TabDrag.abort);
    item.addEventListener('pointerenter', () => TabCard.hover(item));
    item.addEventListener('auxclick', (event) => event.button === 1 && TabStrip.closeByMouse(id));
    item.addEventListener('contextmenu', (event) => TabItem.menu(event, id));
  },

  /** A press: the middle button closes on its click (and must not start autoscroll); the primary one may drag. */
  press(event, item) {
    TabCard.hide();
    if (event.button === 1) return event.preventDefault();
    TabDrag.press(event, item);
  },

  /** The tab's native menu, from the main process. */
  menu(event, id) {
    event.preventDefault();
    TabCard.hide();
    oyaBrowser.showTabMenu(id);
  },

  /** Draws a tab's state: active, failed, title, labels and icon. */
  render(item, tab) {
    item.tabData = tab;
    item.classList.toggle('active', !!tab.active);
    item.classList.toggle('failed', !!tab.loadError);
    TabItem.icon(item, tab);
    TabItem.label(item, tab);
  },

  /** The tab button's name and selection (roving tabindex), and the close button's name. */
  label(item, tab) {
    const title = item.querySelector('.tab-title');
    title.textContent = tab.title || 'New tab';
    title.setAttribute('aria-selected', String(!!tab.active));
    title.tabIndex = tab.active ? 0 : -1;
    item.querySelector('.tab-close').setAttribute('aria-label', 'Close ' + (tab.title || 'tab'));
  },

  /** The icon, redrawn only when what it shows changed, so a favicon never flickers on a title change. */
  icon(item, tab) {
    const key = tab.loading ? 'loading' : tab.home ? 'home' : tab.favicon || 'globe';
    const icon = item.querySelector('.tab-icon');
    if (icon.dataset.key === key) return;
    icon.dataset.key = key;
    icon.replaceChildren();
    if (Object.hasOwn(TAB_ICONS, key)) icon.innerHTML = TAB_ICONS[key]();
    else icon.append(TabItem.favicon(icon, key));
  },

  /** The page's favicon; one that will not decode falls back to the globe. */
  favicon(icon, src) {
    const image = Dom.node('img');
    image.alt = '';
    // An image is draggable by default, and the page's drag-and-drop would cancel the tab's own drag.
    image.draggable = false;
    image.addEventListener('error', () => (icon.innerHTML = ShellIcons.icon('globe')));
    image.src = src;
    return image;
  },
};
